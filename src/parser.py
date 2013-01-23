#==============================================================================
# Parser for Pepper
#==============================================================================

from plyplus import Grammar
import plyplus
import pdb

import compile

FIB_PYTHON = """
def fib(n):
    if n <= 1:
        return 1
    return fib(n-1) + fib(n-2)

for i in range(11):
    print fib(i),
"""

FIB_PEPPER = """
fib(n: Int) =
  1, if n <= 1
  fib(n-1) + fib(n-2), otherwise 
"""

MAIN = """
main() =
  for i <- 1..10
    print(fib(i)) 
"""

POINT = """
Point(x: Int, y: Int) =

   move(dx: Int, dy: Int) = 
     x, y = x+dx, y+dy

   to_string() = "(" x ", " y ")"
"""

DISTANCE = """
s(u: Double, t: Double, a: Double) = u t + 1/2 a t^2
"""

COMPLEX = """
using Complex

z = a + b i
"""

HELLO_PEPPER = """
main() =
  pr("Hello world!")
"""

HELLO_PYTHON = """
def main():
  pr("Hello world!")

"""


# g = Grammar(file("python.g"))
# print g.parse(FIB_PYTHON)
g = Grammar(file("pepper2.g"), debug=True)

# try:
# pdb.set_trace()
ast = g.parse(HELLO_PEPPER)
print ast
compile.compile_ast(ast, "hello")
# print g.parse(HELLO)
#except:
#  pass