type Shape = Circle(float) | Rect(float float)

fn area(s)
    match s
        Circle(r) => 3.14159 * r * r
        Rect(w h) => w * h

c = Circle(5.0)
r = Rect(3.0 4.0)

println(area(c))
println(area(r))
