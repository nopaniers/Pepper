//! Lower the typed executable IR into verified LLVM functions.
use crate::{
    ast::{BinaryOp, UnaryOp},
    semantic::{Expr, ExprKind, Program, Statement, Type},
};
use inkwell::{
    AddressSpace, FloatPredicate, IntPredicate,
    builder::Builder,
    context::Context,
    module::{Linkage, Module},
    targets::TargetMachine,
    types::{BasicMetadataTypeEnum, BasicType, BasicTypeEnum},
    values::{BasicMetadataValueEnum, BasicValueEnum, FunctionValue, IntValue},
};
use std::collections::HashMap;

type Result<T> = std::result::Result<T, String>;

pub(crate) fn generate<'ctx>(
    context: &'ctx Context,
    program: &Program,
    name: &str,
    machine: &TargetMachine,
) -> Result<Module<'ctx>> {
    let module = context.create_module(name);
    module.set_triple(&machine.get_triple());
    module.set_data_layout(&machine.get_target_data().get_data_layout());
    let mut generator = Generator {
        context,
        module,
        builder: context.create_builder(),
        program,
        usize_type: context.ptr_sized_int_type(&machine.get_target_data(), None),
        functions: Vec::new(),
        locals: HashMap::new(),
        current: 0,
    };
    generator.declarations()?;
    for index in 0..program.functions.len() {
        generator.function(index)?;
    }
    generator
        .module
        .verify()
        .map_err(|e| format!("LLVM verification failed: {e}"))?;
    Ok(generator.module)
}

struct Generator<'ctx, 'a> {
    context: &'ctx Context,
    module: Module<'ctx>,
    builder: Builder<'ctx>,
    program: &'a Program,
    usize_type: inkwell::types::IntType<'ctx>,
    functions: Vec<FunctionValue<'ctx>>,
    locals: HashMap<String, BasicValueEnum<'ctx>>,
    current: usize,
}

impl<'ctx> Generator<'ctx, '_> {
    fn ty(&self, ty: &Type) -> Result<BasicTypeEnum<'ctx>> {
        Ok(match ty {
            Type::Int => self.context.i64_type().into(),
            Type::Double => self.context.f64_type().into(),
            Type::Bool => self.context.bool_type().into(),
            Type::String => self
                .context
                .struct_type(
                    &[
                        self.context.ptr_type(AddressSpace::default()).into(),
                        self.usize_type.into(),
                    ],
                    false,
                )
                .into(),
            Type::Array(element, length) => self.ty(element)?.array_type(*length as u32).into(),
            Type::Unit => return Err("Unit has no LLVM value representation".into()),
        })
    }
    fn declarations(&mut self) -> Result<()> {
        let pointer = self.context.ptr_type(AddressSpace::default());
        let i32_type = self.context.i32_type();
        let i64_type = self.context.i64_type();
        let double = self.context.f64_type();
        for (name, parameters) in [
            (
                "pepper_runtime_print_v1",
                vec![pointer.into(), self.usize_type.into()],
            ),
            ("pepper_runtime_print_int_v1", vec![i64_type.into()]),
            ("pepper_runtime_print_double_v1", vec![double.into()]),
            (
                "pepper_runtime_print_bool_v1",
                vec![self.context.i8_type().into()],
            ),
            ("pepper_runtime_error_v1", vec![i32_type.into()]),
        ] {
            self.module
                .add_function(name, i32_type.fn_type(&parameters, false), None);
        }
        self.module.add_function(
            "pepper_runtime_pow_double_v1",
            double.fn_type(&[double.into(), double.into()], false),
            None,
        );
        self.module.add_function(
            "pepper_runtime_pow_int_v1",
            i64_type.fn_type(&[i64_type.into(), i64_type.into()], false),
            None,
        );
        for (index, function) in self.program.functions.iter().enumerate() {
            let mut parameters: Vec<BasicMetadataTypeEnum> = function
                .parameters
                .iter()
                .map(|(_, t)| self.ty(t).map(Into::into))
                .collect::<Result<_>>()?;
            if function.return_type != Type::Unit {
                parameters.push(pointer.into());
            }
            let name = if index == self.program.main {
                "pepper_main".into()
            } else {
                format!("pepper_fn_{index}_{}", function.name)
            };
            let value = self.module.add_function(
                &name,
                i32_type.fn_type(&parameters, false),
                if index == self.program.main {
                    None
                } else {
                    Some(Linkage::Internal)
                },
            );
            self.functions.push(value);
        }
        Ok(())
    }
    fn function(&mut self, index: usize) -> Result<()> {
        self.current = index;
        self.locals.clear();
        let value = self.functions[index];
        self.builder
            .position_at_end(self.context.append_basic_block(value, "entry"));
        let function = &self.program.functions[index];
        for ((name, _), parameter) in function.parameters.iter().zip(value.get_param_iter()) {
            parameter.set_name(name);
            self.locals.insert(name.clone(), parameter);
        }
        for statement in &function.body {
            match statement {
                Statement::Assign(name, expr) => {
                    let value = self.value(expr)?;
                    self.locals.insert(name.clone(), value);
                }
                Statement::Evaluate(expr) => {
                    self.expression(expr)?;
                }
                Statement::Return(expr) => {
                    if let Some(expr) = expr {
                        let result = self.expression(expr)?;
                        if function.return_type != Type::Unit {
                            self.builder
                                .build_store(
                                    value
                                        .get_last_param()
                                        .ok_or("missing result parameter")?
                                        .into_pointer_value(),
                                    result.ok_or("missing return value")?,
                                )
                                .map_err(|e| e.to_string())?;
                        }
                    }
                    self.builder
                        .build_return(Some(&self.context.i32_type().const_zero()))
                        .map_err(|e| e.to_string())?;
                }
            }
        }
        Ok(())
    }
    fn call(
        &self,
        function: FunctionValue<'ctx>,
        arguments: &[BasicMetadataValueEnum<'ctx>],
        name: &str,
    ) -> Result<BasicValueEnum<'ctx>> {
        self.builder
            .build_call(function, arguments, name)
            .map_err(|e| e.to_string())?
            .try_as_basic_value()
            .basic()
            .ok_or_else(|| "expected a non-void call".into())
    }
    fn runtime(
        &self,
        name: &str,
        arguments: &[BasicMetadataValueEnum<'ctx>],
    ) -> Result<BasicValueEnum<'ctx>> {
        self.call(
            self.module
                .get_function(name)
                .ok_or_else(|| format!("missing runtime function {name}"))?,
            arguments,
            "runtime_result",
        )
    }
    fn status(&self, status: IntValue<'ctx>) -> Result<()> {
        let failed = self
            .builder
            .build_int_compare(
                IntPredicate::NE,
                status,
                self.context.i32_type().const_zero(),
                "failed",
            )
            .map_err(|e| e.to_string())?;
        let function = self.functions[self.current];
        let failure = self.context.append_basic_block(function, "error");
        let next = self.context.append_basic_block(function, "next");
        self.builder
            .build_conditional_branch(failed, failure, next)
            .map_err(|e| e.to_string())?;
        self.builder.position_at_end(failure);
        self.builder
            .build_return(Some(&status))
            .map_err(|e| e.to_string())?;
        self.builder.position_at_end(next);
        Ok(())
    }
    fn guard(&self, failed: IntValue<'ctx>, code: u64) -> Result<()> {
        let function = self.functions[self.current];
        let failure = self.context.append_basic_block(function, "runtime_error");
        let next = self.context.append_basic_block(function, "checked");
        self.builder
            .build_conditional_branch(failed, failure, next)
            .map_err(|e| e.to_string())?;
        self.builder.position_at_end(failure);
        let status = self.runtime(
            "pepper_runtime_error_v1",
            &[self.context.i32_type().const_int(code, false).into()],
        )?;
        self.builder
            .build_return(Some(&status))
            .map_err(|e| e.to_string())?;
        self.builder.position_at_end(next);
        Ok(())
    }
    fn value(&mut self, expr: &Expr) -> Result<BasicValueEnum<'ctx>> {
        self.expression(expr)?
            .ok_or_else(|| "Unit used where a value was required".into())
    }
    fn expression(&mut self, expr: &Expr) -> Result<Option<BasicValueEnum<'ctx>>> {
        let value = match &expr.kind {
            ExprKind::Integer(value) => self
                .context
                .i64_type()
                .const_int(*value as u64, true)
                .into(),
            ExprKind::Float(value) => self.context.f64_type().const_float(*value).into(),
            ExprKind::Bool(value) => self
                .context
                .bool_type()
                .const_int(u64::from(*value), false)
                .into(),
            ExprKind::String(text) => {
                let bytes = self.context.const_string(text.as_bytes(), false);
                let global = self.module.add_global(bytes.get_type(), None, "str");
                global.set_initializer(&bytes);
                global.set_constant(true);
                global.set_linkage(Linkage::Private);
                self.context
                    .const_struct(
                        &[
                            global.as_pointer_value().into(),
                            self.usize_type.const_int(text.len() as u64, false).into(),
                        ],
                        false,
                    )
                    .into()
            }
            ExprKind::Local(name) => *self
                .locals
                .get(name)
                .ok_or_else(|| format!("missing local {name}"))?,
            ExprKind::Promote(value) => {
                let value = self.value(value)?.into_int_value();
                self.builder
                    .build_signed_int_to_float(value, self.context.f64_type(), "to_double")
                    .map_err(|e| e.to_string())?
                    .into()
            }
            ExprKind::Array(items) => {
                let mut array = self.ty(&expr.ty)?.into_array_type().const_zero();
                for (index, item) in items.iter().enumerate() {
                    let value = self.value(item)?;
                    array = self
                        .builder
                        .build_insert_value(array, value, index as u32, "array_element")
                        .map_err(|e| e.to_string())?
                        .into_array_value();
                }
                array.into()
            }
            ExprKind::Index(array, index) => {
                let value = self.value(array)?.into_array_value();
                let index = self.value(index)?.into_int_value();
                let Type::Array(_, length) = array.ty else {
                    return Err("expected typed array".into());
                };
                let invalid = self
                    .builder
                    .build_int_compare(
                        IntPredicate::UGE,
                        index,
                        self.context.i64_type().const_int(length as u64, false),
                        "out_of_bounds",
                    )
                    .map_err(|e| e.to_string())?;
                self.guard(invalid, 1)?;
                let slot = self
                    .builder
                    .build_alloca(value.get_type(), "array_copy")
                    .map_err(|e| e.to_string())?;
                self.builder
                    .build_store(slot, value)
                    .map_err(|e| e.to_string())?;
                // SAFETY: the array was stored in slot; the preceding guard checks
                // the signed index (including negatives) before forming its address.
                let address = unsafe {
                    self.builder.build_gep(
                        value.get_type(),
                        slot,
                        &[self.context.i64_type().const_zero(), index],
                        "element",
                    )
                }
                .map_err(|e| e.to_string())?;
                self.builder
                    .build_load(self.ty(&expr.ty)?, address, "indexed")
                    .map_err(|e| e.to_string())?
            }
            ExprKind::Unary(op, operand) => {
                let value = self.value(operand)?;
                match op {
                    UnaryOp::Positive => value,
                    UnaryOp::Not => self
                        .builder
                        .build_not(value.into_int_value(), "not")
                        .map_err(|e| e.to_string())?
                        .into(),
                    UnaryOp::Negate if operand.ty == Type::Double => self
                        .builder
                        .build_float_neg(value.into_float_value(), "negative")
                        .map_err(|e| e.to_string())?
                        .into(),
                    UnaryOp::Negate => self
                        .builder
                        .build_int_neg(value.into_int_value(), "negative")
                        .map_err(|e| e.to_string())?
                        .into(),
                }
            }
            ExprKind::Binary(op, left, right) => {
                let a = self.value(left)?;
                let b = self.value(right)?;
                self.binary(*op, a, b, &left.ty)?
            }
            ExprKind::Print(argument) => {
                let value = self.value(argument)?;
                let status = match argument.ty {
                    Type::String => {
                        let value = value.into_struct_value();
                        let data = self
                            .builder
                            .build_extract_value(value, 0, "string_data")
                            .map_err(|e| e.to_string())?;
                        let length = self
                            .builder
                            .build_extract_value(value, 1, "string_length")
                            .map_err(|e| e.to_string())?;
                        self.runtime("pepper_runtime_print_v1", &[data.into(), length.into()])?
                    }
                    Type::Int => self.runtime("pepper_runtime_print_int_v1", &[value.into()])?,
                    Type::Double => {
                        self.runtime("pepper_runtime_print_double_v1", &[value.into()])?
                    }
                    Type::Bool => {
                        let byte = self
                            .builder
                            .build_int_z_extend(
                                value.into_int_value(),
                                self.context.i8_type(),
                                "bool_byte",
                            )
                            .map_err(|e| e.to_string())?;
                        self.runtime("pepper_runtime_print_bool_v1", &[byte.into()])?
                    }
                    _ => return Err("unsupported print argument".into()),
                };
                self.status(status.into_int_value())?;
                return Ok(None);
            }
            ExprKind::Call(index, arguments) => {
                let function = &self.program.functions[*index];
                let result_type = function.return_type.clone();
                let mut values = Vec::new();
                for argument in arguments {
                    values.push(self.value(argument)?.into());
                }
                let result = if result_type == Type::Unit {
                    None
                } else {
                    Some(
                        self.builder
                            .build_alloca(self.ty(&result_type)?, "call_result")
                            .map_err(|e| e.to_string())?,
                    )
                };
                if let Some(slot) = result {
                    values.push(slot.into());
                }
                let status = self
                    .call(self.functions[*index], &values, "call_status")?
                    .into_int_value();
                self.status(status)?;
                return result
                    .map(|slot| {
                        self.builder
                            .build_load(self.ty(&result_type)?, slot, "result")
                            .map_err(|e| e.to_string())
                    })
                    .transpose();
            }
        };
        Ok(Some(value))
    }

    fn binary(
        &self,
        op: BinaryOp,
        a: BasicValueEnum<'ctx>,
        b: BasicValueEnum<'ctx>,
        ty: &Type,
    ) -> Result<BasicValueEnum<'ctx>> {
        use BinaryOp::*;
        if *ty == Type::Double {
            let a = a.into_float_value();
            let b = b.into_float_value();
            let predicate = match op {
                Equal => Some(FloatPredicate::OEQ),
                NotEqual => Some(FloatPredicate::UNE),
                Less => Some(FloatPredicate::OLT),
                LessEqual => Some(FloatPredicate::OLE),
                Greater => Some(FloatPredicate::OGT),
                GreaterEqual => Some(FloatPredicate::OGE),
                _ => None,
            };
            if let Some(predicate) = predicate {
                return self
                    .builder
                    .build_float_compare(predicate, a, b, "compare")
                    .map(Into::into)
                    .map_err(|e| e.to_string());
            }
            return match op {
                Add => self.builder.build_float_add(a, b, "add"),
                Subtract => self.builder.build_float_sub(a, b, "subtract"),
                Multiply => self.builder.build_float_mul(a, b, "multiply"),
                Divide => self.builder.build_float_div(a, b, "divide"),
                Remainder => self.builder.build_float_rem(a, b, "remainder"),
                Power => {
                    return self.runtime("pepper_runtime_pow_double_v1", &[a.into(), b.into()]);
                }
                _ => return Err("unsupported floating operator".into()),
            }
            .map(Into::into)
            .map_err(|e| e.to_string());
        }
        let a = a.into_int_value();
        let b = b.into_int_value();
        let predicate = match op {
            Equal => Some(IntPredicate::EQ),
            NotEqual => Some(IntPredicate::NE),
            Less => Some(IntPredicate::SLT),
            LessEqual => Some(IntPredicate::SLE),
            Greater => Some(IntPredicate::SGT),
            GreaterEqual => Some(IntPredicate::SGE),
            _ => None,
        };
        if let Some(predicate) = predicate {
            return self
                .builder
                .build_int_compare(predicate, a, b, "compare")
                .map(Into::into)
                .map_err(|e| e.to_string());
        }
        if matches!(op, Divide | Remainder) {
            let zero = self
                .builder
                .build_int_compare(
                    IntPredicate::EQ,
                    b,
                    self.context.i64_type().const_zero(),
                    "zero_divisor",
                )
                .map_err(|e| e.to_string())?;
            let min = self
                .builder
                .build_int_compare(
                    IntPredicate::EQ,
                    a,
                    self.context.i64_type().const_int(i64::MIN as u64, true),
                    "minimum",
                )
                .map_err(|e| e.to_string())?;
            let negative_one = self
                .builder
                .build_int_compare(
                    IntPredicate::EQ,
                    b,
                    self.context.i64_type().const_all_ones(),
                    "negative_one",
                )
                .map_err(|e| e.to_string())?;
            let overflow = self
                .builder
                .build_and(min, negative_one, "division_overflow")
                .map_err(|e| e.to_string())?;
            let invalid = self
                .builder
                .build_or(zero, overflow, "invalid_division")
                .map_err(|e| e.to_string())?;
            self.guard(invalid, 2)?;
        }
        if op == Power {
            let negative = self
                .builder
                .build_int_compare(
                    IntPredicate::SLT,
                    b,
                    self.context.i64_type().const_zero(),
                    "negative_exponent",
                )
                .map_err(|e| e.to_string())?;
            self.guard(negative, 3)?;
            return self.runtime("pepper_runtime_pow_int_v1", &[a.into(), b.into()]);
        }
        match op {
            Add => self.builder.build_int_add(a, b, "add"),
            Subtract => self.builder.build_int_sub(a, b, "subtract"),
            Multiply => self.builder.build_int_mul(a, b, "multiply"),
            Divide => self.builder.build_int_signed_div(a, b, "divide"),
            Remainder => self.builder.build_int_signed_rem(a, b, "remainder"),
            _ => return Err("unsupported integer operator".into()),
        }
        .map(Into::into)
        .map_err(|e| e.to_string())
    }
}
