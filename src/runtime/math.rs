use libc::c_double;

#[unsafe(no_mangle)]
pub extern "C" fn toy_sin(x: c_double) -> c_double {
    x.sin()
}

#[unsafe(no_mangle)]
pub extern "C" fn toy_cos(x: c_double) -> c_double {
    x.cos()
}

#[unsafe(no_mangle)]
pub extern "C" fn toy_tan(x: c_double) -> c_double {
    x.tan()
}

#[unsafe(no_mangle)]
pub extern "C" fn toy_sqrt(x: c_double) -> c_double {
    x.sqrt()
}

#[unsafe(no_mangle)]
pub extern "C" fn toy_pow(base: c_double, exp: c_double) -> c_double {
    base.powf(exp)
}

#[unsafe(no_mangle)]
pub extern "C" fn toy_exp(x: c_double) -> c_double {
    x.exp()
}

#[unsafe(no_mangle)]
pub extern "C" fn toy_log(x: c_double) -> c_double {
    x.ln()
}

#[unsafe(no_mangle)]
pub extern "C" fn toy_ceil(x: c_double) -> c_double {
    x.ceil()
}

#[unsafe(no_mangle)]
pub extern "C" fn toy_floor(x: c_double) -> c_double {
    x.floor()
}
