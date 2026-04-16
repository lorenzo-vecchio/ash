fn fib(n)
    if n <= 1
        n
    else
        fib(n - 1) + fib(n - 2)

println(fib(0))
println(fib(1))
println(fib(5))
println(fib(10))
