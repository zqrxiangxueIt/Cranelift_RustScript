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

impl TypeChecker {
    pub fn new() -> Self {
        let mut tc = TypeChecker {
            functions: HashMap::new(),
        };
        tc.register_builtins();
        tc
    }

    fn register_builtins(&mut self) {
        let math_unary = vec![
            "sin", "cos", "tan", "sqrt", "exp", "log", "ceil", "floor"
        ];
        for name in math_unary {
            self.functions.insert(name.to_string(), FunctionSignature {
                params: vec![Type::F64],
                ret: Type::F64,
                is_external: true,
            });
        }
        
        self.functions.insert("pow".to_string(), FunctionSignature {
            params: vec![Type::F64, Type::F64],
            ret: Type::F64,
            is_external: true,
        });

        self.functions.insert("putchar".to_string(), FunctionSignature {
            params: vec![Type::I64],
            ret: Type::I64,
            is_external: true,
        });
        
        self.functions.insert("rand".to_string(), FunctionSignature {
            params: vec![],
            ret: Type::I64,
            is_external: true,
        });
        
        // External functions for nalgebra demo
        // sum_array: fn(arr: [f64; N]) -> f64
        // We use a special placeholder or just allow it in validation logic
        self.functions.insert("sum_array".to_string(), FunctionSignature {
            // We'll allow any array of F64 in logic, but here we might need a way to express it.
            // For now, assume it's unchecked here or checked dynamically
            params: vec![], // Empty params as wildcard? Or strict? 
            // Let's rely on name-based checking for arrays for now since Type system is limited
            ret: Type::F64,
            is_external: true,
        });

        self.functions.insert("print_matrix_2x2".to_string(), FunctionSignature {
            params: vec![], // Wildcard
            ret: Type::I64, // Void-ish (returns 0 or something)
            is_external: true,
        });
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
        },
        Expr::Cast(_, ty) => ty.clone(),
        Expr::Add(lhs, _) | Expr::Sub(lhs, _) | Expr::Mul(lhs, _) | Expr::Div(lhs, _) => {
            infer_type(lhs, get_var_type)
        },
        Expr::Eq(_, _) | Expr::Ne(_, _) | Expr::Lt(_, _) | Expr::Le(_, _) | Expr::Gt(_, _) | Expr::Ge(_, _) => {
            Type::I64 // Booleans are I64 (0 or 1)
        },
        Expr::Identifier(name) => {
             get_var_type(name).unwrap_or(Type::I64)
        },
        Expr::Call(_, _) => {
            // Fallback, ideally we look up function signature
            Type::I64 
        },
        Expr::Index(base, _) => {
             match infer_type(base, get_var_type) {
                 Type::Array(inner, _) => *inner,
                 _ => Type::I64,
             }
        },
        Expr::Assign(_, expr) => infer_type(expr, get_var_type),
        Expr::IfElse(_, then_body, _) => {
            if let Some(last) = then_body.last() {
                infer_type(last, get_var_type)
            } else {
                Type::I64
            }
        },
        Expr::WhileLoop(_, _) => Type::I64,
        Expr::GlobalDataAddr(_) => Type::I64, // Pointer
    }
}
