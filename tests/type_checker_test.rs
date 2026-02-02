use cranelift_jit_demo::type_checker::{TypeChecker, infer_type};
use cranelift_jit_demo::frontend::{Expr, Type};

#[test]
fn test_resolve_func() {
    let tc = TypeChecker::new();
    let sig = tc.resolve_func("sin").unwrap();
    assert_eq!(sig.params, vec![Type::F64]);
    assert_eq!(sig.ret, Type::F64);
    assert!(sig.is_external);
}

#[test]
fn test_infer_type_simple() {
    let expr = Expr::Literal("123".to_string(), Type::I64);
    let ty = infer_type(&expr, &|_| None);
    assert_eq!(ty, Type::I64);
}

#[test]
fn test_infer_type_array() {
    let expr = Expr::ArrayLiteral(vec![
        Expr::Literal("1.0".to_string(), Type::F64),
        Expr::Literal("2.0".to_string(), Type::F64)
    ], Type::I64); // Placeholder type
    
    let ty = infer_type(&expr, &|_| None);
    if let Type::Array(inner, len) = ty {
        assert_eq!(*inner, Type::F64);
        assert_eq!(len, 2);
    } else {
        panic!("Expected Array type");
    }
}
