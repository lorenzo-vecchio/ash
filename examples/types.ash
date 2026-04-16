type Shape = Circle(float) | Rect(float float) | Triangle(float float float)

fn area(s)
    match s
        Circle(r)        => math.pi * r * r
        Rect(w h)        => w * h
        Triangle(a b c)  => 0.5 * a * b

fn describe(s)
    match s
        Circle(r)       => "circle with radius {r}"
        Rect(w h)       => "rectangle {w}x{h}"
        Triangle(a b c) => "triangle with sides {a} {b} {c}"

shapes = [Circle(5.0), Rect(3.0, 4.0), Triangle(3.0, 4.0, 5.0)]

for s in shapes
    println(describe(s))
    println(area(s))
