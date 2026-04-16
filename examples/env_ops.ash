home = env.get("HOME")
println(home ?? "no HOME set")

missing = env.get("__ASH_TOTALLY_MISSING__")
println(missing ?? "not set")
