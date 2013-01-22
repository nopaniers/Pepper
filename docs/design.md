Pepper Language Design
======================

Pepper is a modern systems programming language. It aims to replicate the best of all worlds. Some of the core priorities of the language are

- Interperability with existing C code
- Type inference
- Functional constructs
- Coroutines out of the box
- Object orientated
- A great standard library
- Whitespace (like in Python) is significant, but {} are also allowed
- Conventions enforced by compiler

A brief taste of Pepper is the following,

    main() =
       print("Hello world!")

and that's it. Easy! Enjoy!


Comments
--------

Comments are everything following the hash sign.

    # This is a comment


Variables
---------

The following declares and Intializes variables,

    x = 1
    y = 2
    str = "I am a string!"

They receive their type from what they are assigned to. This is illegal,

    x = 1
    x = "Changing types is not cool!"  # Error

The parameters of a function must be specified,

    add(x: Int, y: Int) = x + y

But the return type can be inferred from the last expression, or the return expression.


Functions
---------

Functions are call by value only. Classes are required if you want to have a side-effect on the data.


Native Types
------------

The native types correspond to different (regularly available) types on machines.

    x : Int = 1
    y : Unsigned = 2
    c : Char = 'c'
    s : String = "This is a string"

Arrays are easy to specify,

    arr = [1,2,3]
    double_arry : Int[][] = [[4,5,6], [1,2,3], [5,6,7]]

and can easily be appended to (even if this takes time)

    arr = arr + [5]

or

    arr.append(5)

Slices work like this

    arr[3..5] = [3, 4, 5]

Hashes are much the same as well,

    hash["one"] = 1
    hash : Int[String] = {}
    hash = { "one" -> 1, "two" -> 2 }

and can be accessed via

    hash.one

Finally, multiple assignment is fine

    a, b = c, d



Packages
--------

Packages are defined by the directory layout, from an environment variable, PEPPER_PATH, and also from the current directory. These can be imported with the import statement

    uses sys.Socket
    uses io.*


Classes
-------

Everything is a class. Methods on classes are just functions that take (a poInter) to the class as the first parameter. That is,

    circle.area()

is the same as

    area(circle)


A class is defined in the normal way, but classes start with capitals

    Point(x: Int, y: Int) =

       move(dx: Int, dy: Int) = 
         x, y = x+dx, y+dy

       to_string() = "(" x ", " y ")"


Notice that parameters passed to the class are automatically made attributes.We can create an instance of this class by

    point = Point(3, 4)


Code Blocks
-----------

It should be easy to make code blocks, as it is in Ruby. For example, on an array, it should be easy to do

    for book <- books
      print(book.title " was written by " book.name)

    books.each (book)
      print(book.title " was written by " book.name)

Or

    titles = books.map (book)
      book.title

    titles = [book.title for book <- books]



Searching
---------

It should be very easy to filter arrays,

    best_books = books[author == "Kipling" and "poem" not in title]
    odd_nums = nums[x%2 == 1]


Operator Overloading
--------------------

Yes. Just write a class which defines

    Matrix(rows: Int, cols: Int) = 

       add(that: Matrix) =
          sum = Matrix(rows, cols)
          for (row<-0..rows) 
            for (col<-0..cols) 
              sum[row][col] = this[row][col]+that[row][col]
          return(sum)


Space can be significant
------------------------

For example, strings can be concatenated this way.


For loops
---------

For loops are over an array or an iterator

    for x <- 1..10

While loops
-----------

    x=0
    while x<10
      print(x)
      x = x+1


With blocks
-----------

Automatically call whatever tidy up code is needed,

    with file = open("myfile.pepper")
      .. do stuff ..


Error Handling
--------------

As per normal. Try/catch/finally

    try
       x = open("pesky_file.txt")
    
    catch (e:Error)
       print(e)

    finally
       x.close()

Can easily throw errors with

    assert(x>0, "Don't give me any of that negative number stuff")

Or just create an error object to throw it

    Error("Yeah. This is an error and there's nothing you can do about it")



Interoperable with C
--------------------

We need to be able to Interact with C. One thing we need is specifying pointers. Classes use pointers behind the scenes. The following

    inc(x: PoInterTo(Int)) = 
      x = x+1

Eventually able to read .h files, and write them. Automatically route types to the C equivalents and back again when talking to c code. Potentially use clang for this.

    include stdio

    main() =
      printf("Hello World from C!\n")


Coroutines
----------

Have go-style coroutines for parallelism. Can make things multithreaded anywhere anytime just by adding co to a constuctor or a function.

    co class ....
    co hello()
    co Thread()

Communication happens via messages passed between the two.

   message << stream   # Reading from stream
   message >> stream   # Writing to a stream

Blocks should be able to be used on GPU if they're available, for example.


Casting
-------

   1.0 Int
   1 Double

or equivalently

   Int(1.0)
   Double(1)


Unit Testing
------------

Preconditions, invariants, postconditions and unit tests should be expressable as part of the language. This can be done by simply creating subfunctions named correctly as part of a method, class or function.

For example,

  Account(balance: Int) =

    withdraw(amount: Int) = 
      balance = balance - amount

      test() = 
        account = Account(100)
        account.withdraw(10)
        assert(balance == 90)

      precondition() =
        initial_balance = balance
        assert(amount >= 0)

      postcondition() =
        balance < initial_balance


Getting and Setting
-------------------

It should be easy to override get and set methods

  Example() =

    parameter : Int
      get() =
        return(parameter)

      set(value) = 
        parameter = value

Writing things in this way *overwrites* (or creates if none exists) the method in the corresponding class.


Nullable
--------

Hmmmm... At the moment I'm thinking if a variable can be null, it has to have the "?" operator. For example,

  value : Int?
  x?.y?.z   # Swallows the null dereference, returning null if it is.


Type system
===========

Actually I prefer to think of types two slightly different ideas, *constraints* and *interfaces*.

A variable can have several contraints on it, and can also satisfy many different types of interfaces. 

  - If a class specifies the right methods, or satisfies a constraint it *is* of that type.

  - For example a class with specifies first() and next() methods can be and Iterator *regardless* of it it explicitly subclasses Iterator or not.

  - Another example, a prime integer, 5, is in the class Prime, whether or not it explicitly subclasses Prime.

  - A normal class declaration expresses both of these two concerns. It specifies constraints, an interface and it can specify all or part of the implementation.

  - You can explicitly declare a variable to be of one or other types, which gives it constraints. It will *always satisfy* these constraints and interface.

  - You can add constraints and interfaces explicitly.


Constraints

So we make a class representing a particular range,

  InRange(from: Int, to: Int) = 
    ensure =
      this <- Int 
      this >= from
      this <= to

  x : Int, InRange(10, 20) = 15

Or even on the fly, we could add constraints,

  x = 2
  x = x+10
  x: InRange(10, 20)  # From here on in x is always in this range, and the compiler should be able to check this.

This lets us, for example, define

  u = 5 M/S
  t = 1 S

  s = u t

After we define the classes M, S these result in the calls
  M(5), S()
automatically constructing classes and checking units. Such classes don't actually are about the type which is being passed around (in this case it's an Int). They just serve to keep track of the units.

So

M(this) = 
  equal(lhs)
    lhs : M # Pass the constraint to things assigned to this
    this.equal(lhs)

Classes like this work like annotations telling the compiler things it can (and should check).


It's possible to specify interfaces, which has virtual methods,

  File(filename: String) =
    open(String)
    close(String)
  
And which have to be over-written by other classes.

Enumerations can be written like this

  DaysOftheWeek = 
    Monday
    Tuesday
    Wednesday
    Thursday
    Friday
    Saturday
    Sunday

We inherit by "using" just the same as we do at module level.

A() =
  uses B
  uses C.do_something_else

If there's a conflict we can resolve with

C() =
  uses B.whoami as b_whoami



For Physical Simulation
=======================

Spaces are significant
----------------------

Matrices should be easy to specify, as should complex numbers. The easiest way to do this is to make space (" ") significant. So we can do

s(u, t: Double) = u t

where times is understood. We can also do

s = 3 M
u = 3 M/S

and get correct units (which can be checked at compile time).

But we can also do complex numbers

z = a + b i


Matrices, Vectors and Tensors in Standard Library
-------------------------------------------------

mat = [[0, 1],
       [1, 0]] Matrix

down = [0, 1] Vector

or 

vec = Vector([0, 1])


A lot of standard operations
----------------------------

Complex and integer arrays *are* matrices and vectors. Standard linear algebra operations are defined on them.

