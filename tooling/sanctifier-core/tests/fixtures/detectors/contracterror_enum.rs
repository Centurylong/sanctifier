pub enum GoodError {
    A,
}
#[contracterror]
#[repr(u32)]
pub enum GoodError2 {
    B,
}

#[contracterror]
pub enum BadError1 {
    C,
}

#[repr(u32)]
pub enum BadError2 {
    D,
}

pub fn some_func() -> Result<(), GoodError> {
    Ok(())
}

pub fn some_func2() -> Result<(), GoodError2> {
    Ok(())
}

pub fn some_func3() -> Result<(), BadError1> {
    Ok(())
}

pub fn some_func4() -> Result<(), BadError2> {
    Ok(())
}
