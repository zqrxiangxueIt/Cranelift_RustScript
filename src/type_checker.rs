use crate::frontend::{Expr, Type};
use std::collections::HashMap;

#[derive(Clone, Debug)]
pub struct FunctionSignature {
    pub params: Vec<Type>,
    pub ret: Type,
    pub is_external: bool,
}

pub struct TypeChecker {
    pub functions: HashMap<String, FunctionSignature>,
}

impl Default for TypeChecker {
    fn default() -> Self {
        Self::new()
    }
}

impl TypeChecker {
    pub fn new() -> Self {
        let mut tc = TypeChecker {
            functions: HashMap::new(),
        };
        tc.register_builtins();
        tc
    }

    fn register_builtins(&mut self) {
        let math_unary = vec!["sin", "cos", "tan", "sqrt", "exp", "log", "ceil", "floor"];
        for name in math_unary {
            self.functions.insert(
                name.to_string(),
                FunctionSignature {
                    params: vec![Type::F64],
                    ret: Type::F64,
                    is_external: true,
                },
            );
        }

        self.functions.insert(
            "pow".to_string(),
            FunctionSignature {
                params: vec![Type::F64, Type::F64],
                ret: Type::F64,
                is_external: true,
            },
        );

        self.functions.insert(
            "putchar".to_string(),
            FunctionSignature {
                params: vec![Type::I64],
                ret: Type::I64,
                is_external: true,
            },
        );

        self.functions.insert(
            "rand".to_string(),
            FunctionSignature {
                params: vec![],
                ret: Type::I64,
                is_external: true,
            },
        );

        self.functions.insert(
            "toy_sum_array".to_string(),
            FunctionSignature {
                params: vec![Type::Array(Box::new(Type::F64), 0)],
                ret: Type::F64,
                is_external: true,
            },
        );

        self.functions.insert(
            "print_f64".to_string(),
            FunctionSignature {
                params: vec![Type::F64],
                ret: Type::F64,
                is_external: true,
            },
        );

        self.functions.insert(
            "print_i64".to_string(),
            FunctionSignature {
                params: vec![Type::I64],
                ret: Type::I64,
                is_external: true,
            },
        );

        // DynamicArray methods
        self.functions.insert(
            "array_push".to_string(),
            FunctionSignature {
                params: vec![Type::DynamicArray(Box::new(Type::I64)), Type::I64],
                ret: Type::I64,
                is_external: true,
            },
        );
        self.functions.insert(
            "array_pop".to_string(),
            FunctionSignature {
                params: vec![Type::DynamicArray(Box::new(Type::I64))],
                ret: Type::I64,
                is_external: true,
            },
        );
        self.functions.insert(
            "array_len".to_string(),
            FunctionSignature {
                params: vec![Type::DynamicArray(Box::new(Type::I64))],
                ret: Type::I64,
                is_external: true,
            },
        );
        self.functions.insert(
            "array_cap".to_string(),
            FunctionSignature {
                params: vec![Type::DynamicArray(Box::new(Type::I64))],
                ret: Type::I64,
                is_external: true,
            },
        );
        self.functions.insert(
            "array_set".to_string(),
            FunctionSignature {
                params: vec![
                    Type::DynamicArray(Box::new(Type::I64)),
                    Type::I64,
                    Type::I64,
                ],
                ret: Type::I64,
                is_external: true,
            },
        );

        // F64 DynamicArray methods
        self.functions.insert(
            "array_new_f64".to_string(),
            FunctionSignature {
                params: vec![],
                ret: Type::DynamicArray(Box::new(Type::F64)),
                is_external: true,
            },
        );
        self.functions.insert(
            "array_push_f64".to_string(),
            FunctionSignature {
                params: vec![Type::DynamicArray(Box::new(Type::F64)), Type::F64],
                ret: Type::I64,
                is_external: true,
            },
        );
        self.functions.insert(
            "array_pop_f64".to_string(),
            FunctionSignature {
                params: vec![Type::DynamicArray(Box::new(Type::F64))],
                ret: Type::F64,
                is_external: true,
            },
        );
        self.functions.insert(
            "array_len_f64".to_string(),
            FunctionSignature {
                params: vec![Type::DynamicArray(Box::new(Type::F64))],
                ret: Type::I64,
                is_external: true,
            },
        );
        self.functions.insert(
            "array_cap_f64".to_string(),
            FunctionSignature {
                params: vec![Type::DynamicArray(Box::new(Type::F64))],
                ret: Type::I64,
                is_external: true,
            },
        );
        self.functions.insert(
            "array_set_f64".to_string(),
            FunctionSignature {
                params: vec![
                    Type::DynamicArray(Box::new(Type::F64)),
                    Type::I64,
                    Type::F64,
                ],
                ret: Type::I64,
                is_external: true,
            },
        );

        // Complex128 DynamicArray methods
        self.functions.insert(
            "array_new_complex128".to_string(),
            FunctionSignature {
                params: vec![],
                ret: Type::DynamicArray(Box::new(Type::Complex128)),
                is_external: true,
            },
        );
        self.functions.insert(
            "array_push_complex128".to_string(),
            FunctionSignature {
                params: vec![Type::DynamicArray(Box::new(Type::Complex128)), Type::Complex128],
                ret: Type::I64,
                is_external: true,
            },
        );
        self.functions.insert(
            "array_pop_complex128".to_string(),
            FunctionSignature {
                params: vec![Type::DynamicArray(Box::new(Type::Complex128))],
                ret: Type::Complex128,
                is_external: true,
            },
        );
        self.functions.insert(
            "array_len_complex128".to_string(),
            FunctionSignature {
                params: vec![Type::DynamicArray(Box::new(Type::Complex128))],
                ret: Type::I64,
                is_external: true,
            },
        );
        self.functions.insert(
            "array_cap_complex128".to_string(),
            FunctionSignature {
                params: vec![Type::DynamicArray(Box::new(Type::Complex128))],
                ret: Type::I64,
                is_external: true,
            },
        );
        self.functions.insert(
            "array_set_complex128".to_string(),
            FunctionSignature {
                params: vec![
                    Type::DynamicArray(Box::new(Type::Complex128)),
                    Type::I64,
                    Type::Complex128,
                ],
                ret: Type::I64,
                is_external: true,
            },
        );

        // Register toy_mkl_dgemm
        // fn toy_mkl_dgemm(
        //     m: i64, n: i64, k: i64,
        //     alpha: f64, a: [f64],
        //     beta: f64, b: [f64],
        //     c: [f64]
        // ) -> void
        // Note: Arrays in JIT are passed as (ptr, len), so we need to match that expectation
        // or ensure the caller passes arrays which will be expanded.
        // The toy language expands arrays to (ptr, len) automatically for external calls.
        // So signature here should use Array type if we want that expansion logic to trigger.
        self.functions.insert(
            "toy_mkl_dgemm".to_string(),
            FunctionSignature {
                params: vec![
                    Type::I64,
                    Type::I64,
                    Type::I64, // m, n, k
                    Type::F64,
                    Type::Array(Box::new(Type::F64), 0), // alpha, a (size ignored)
                    Type::F64,
                    Type::Array(Box::new(Type::F64), 0), // beta, b
                    Type::Array(Box::new(Type::F64), 0), // c
                ],
                ret: Type::I64, // void
                is_external: true,
            },
        );
    }

    pub fn resolve_func(&self, name: &str) -> Option<&FunctionSignature> {
        self.functions.get(name)
    }
}

/// Infer type of expression.
/// `get_var_type` is a callback to look up variable types from the current scope.
pub fn infer_type(expr: &Expr, get_var_type: &impl Fn(&str) -> Option<Type>) -> Type {
    match expr {
        Expr::Literal(_, ty) => ty.clone(),
        Expr::StringLiteral(_) => Type::String,
        Expr::ComplexLiteral(_, _, ty) => ty.clone(),
        Expr::ArrayLiteral(elems, _) => {
            if elems.is_empty() {
                Type::Array(Box::new(Type::I64), 0)
            } else {
                let elem_ty = infer_type(&elems[0], get_var_type);
                Type::Array(Box::new(elem_ty), elems.len())
            }
        }
        Expr::DynamicArrayLiteral(elems, _) => {
            if elems.is_empty() {
                Type::DynamicArray(Box::new(Type::I64))
            } else {
                let elem_ty = infer_type(&elems[0], get_var_type);
                Type::DynamicArray(Box::new(elem_ty))
            }
        }
        Expr::Cast(_, ty) => ty.clone(),
        Expr::Add(lhs, _) | Expr::Sub(lhs, _) | Expr::Mul(lhs, _) | Expr::Div(lhs, _) => {
            infer_type(lhs, get_var_type)
        }
        Expr::Eq(_, _)
        | Expr::Ne(_, _)
        | Expr::Lt(_, _)
        | Expr::Le(_, _)
        | Expr::Gt(_, _)
        | Expr::Ge(_, _) => {
            Type::I64 // Booleans are I64 (0 or 1)
        }
        Expr::Identifier(name) => get_var_type(name).unwrap_or(Type::I64),
        Expr::Call(name, _) => {
            // 查表获取函数返回类型，而不是硬编码
            match name.as_str() {
                // 数学函数 -> F64
                "sin" | "cos" | "tan" | "sqrt" | "exp" | "log" | "ceil" | "floor" | "pow" => {
                    Type::F64
                }
                // IO 函数
                "putchar" | "rand" | "printf" | "puts" | "toy_sum_array" => Type::I64,
                "print_f64" => Type::F64,
                "print_i64" => Type::I64,
                // toy_mkl_dgemm 返回 i64 (错误码)
                "toy_mkl_dgemm" => Type::I64,
                // i64 动态数组方法 -> I64
                "array_push" | "array_pop" | "array_len" | "array_cap" | "array_set" => Type::I64,
                // i64 动态数组构造函数
                "array_new_i64" => Type::DynamicArray(Box::new(Type::I64)),
                // f64 动态数组方法
                "array_new_f64" => Type::DynamicArray(Box::new(Type::F64)),
                "array_push_f64" | "array_len_f64" | "array_cap_f64" | "array_set_f64" => Type::I64,
                "array_pop_f64" => Type::F64,
                // complex128 动态数组方法
                "array_new_complex128" => Type::DynamicArray(Box::new(Type::Complex128)),
                "array_push_complex128" | "array_len_complex128" | "array_cap_complex128" | "array_set_complex128" => Type::I64,
                "array_pop_complex128" => Type::Complex128,
                // 未知函数默认返回 I64
                _ => Type::I64,
            }
        }
        Expr::Index(base, _) => match infer_type(base, get_var_type) {
            Type::Array(inner, _) => *inner,
            Type::DynamicArray(inner) => *inner,
            _ => Type::I64,
        },
        Expr::Assign(_, expr) => infer_type(expr, get_var_type),
        Expr::IfElse(_, then_body, _) => {
            if let Some(last) = then_body.last() {
                infer_type(last, get_var_type)
            } else {
                Type::I64
            }
        }
        Expr::WhileLoop(_, _) => Type::I64,
        Expr::GlobalDataAddr(_) => Type::I64, // Pointer
        Expr::Drop(_) => Type::I64, // drop() 不返回有用值
    }
}
