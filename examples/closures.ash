fn make_adder(n)
    x => x + n

add10 = make_adder(10)
add20 = make_adder(20)

println(add10(5))
println(add20(5))
