type Result = Ok(int) | Err(str)

fn safe_div(a b)
    if b == 0
        Err("division by zero")
    else
        Ok(a / b)

r1 = safe_div(10, 2)
r2 = safe_div(5, 0)

match r1
    Ok(v)  => println(v)
    Err(e) => println(e)

match r2
    Ok(v)  => println(v)
    Err(e) => println(e)
