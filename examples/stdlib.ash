# math namespace
println(math.pi)
println(math.sqrt(16.0))
println(math.floor(3.9))
println(math.pow(2.0, 10.0))

# higher-order functions
nums = [1, 2, 3, 4, 5]
doubled = map(nums, x => x * 2)
println(doubled)

total = reduce(nums, (acc x) => acc + x, 0)
println(total)

has_big = any(nums, x => x > 4)
all_pos  = all(nums, x => x > 0)
println(has_big)
println(all_pos)

# zip and flat
pairs = zip([1, 2, 3], ["a", "b", "c"])
println(pairs)

nested = [[1, 2], [3, 4], [5]]
flat_list = flat(nested)
println(flat_list)

# clamp
println(clamp(15, 0, 10))
println(clamp(-5, 0, 10))
println(clamp(5, 0, 10))
