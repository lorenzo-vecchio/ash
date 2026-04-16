nums = [1, 2, 3, 4, 5, 6, 7, 8]

evens   = nums |> filter(x => x % 2 == 0)
doubled = evens |> map(x => x * 2)

for n in doubled
    println(n)
