start: block;

//     STATEMENTS

block: (NEWLINE|stmt)@+;

stmt: PASS
  | function_definition
  | class_definition;


function_definition: IDENTIFIER OPEN parameter_list? CLOSE EQUALS block;

parmeter: IDENTIFIER COLON type;
parameter_list: parameter (, parameter)*;


type: CLASS_NAME;



// Lexical rules
PASS: 'pass';

OPEN: '\(';
CLOSE: '\)';
EQUALS: '=';
COLON: ':';

IDENTIFIER: '[a-z_][a-zA-Z_0-9]*(?!r?"|r?\')';
CLASS_NAME: '[A-Z][a-zA-Z_0-9]*(?!r?"|r?\')';

NEWLINE: '(\r?\n[\t ]*)+'    // Don't count on the + to prevent multiple NEWLINE tokens. It's just an optimization
    (%newline)
    ;

%newline_char: '\n';    // default, can be omitted

###
from grammars.python_indent_postlex import PythonIndentTracker
self.lexer_postproc = PythonIndentTracker

