use std::{collections::HashMap, fmt, rc::Rc};

use crate::parser::{AstNode, Function, Node, Op, Position};

#[derive(Clone, Debug)]
pub enum Value {
    Empty,
    Bool(bool),
    Number(f64),
    Array(Rc<Vec<Value>>),
    Range { min: isize, max: isize },
    StrLiteral(Rc<String>),
    // Str(String),
    Function(Rc<Function>),
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        use Value::*;
        match self {
            Empty => write!(f, "()"),
            Bool(s) => write!(f, "{}", s),
            Number(n) => write!(f, "{}", n),
            StrLiteral(s) => write!(f, "{}", s),
            Array(a) => {
                write!(f, "[")?;
                for (i, value) in a.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{}", value)?;
                }
                write!(f, "]")
            }
            Range { min, max } => write!(f, "[{}..{}]", min, max),
            Function(function) => {
                let raw = Rc::into_raw(function.clone());
                write!(f, "function: {:?}", raw)
            }
        }
    }
}

struct Scope {
    values: HashMap<Rc<String>, Value>,
}

impl Scope {
    fn new() -> Self {
        Self {
            values: HashMap::new(),
        }
    }
}

macro_rules! runtime_error {
    ($position:expr, $error:expr) => {
        Err(format!(
            "Runtime error at line: {} column: {}\n - {}",
            $position.line, $position.column, $error
        ))
    };
}

pub struct Runtime {
    _global: Scope,
}

impl Runtime {
    pub fn new() -> Self {
        Self {
            _global: Scope::new(),
        }
    }

    pub fn run(&mut self, ast: &Vec<AstNode>) -> Result<Value, String> {
        self.evaluate_block(ast, &mut Scope::new())
    }

    fn evaluate_block(&mut self, block: &Vec<AstNode>, scope: &mut Scope) -> Result<Value, String> {
        let mut result = Value::Empty;
        for node in block.iter() {
            result = self.evaluate(node, scope)?;
        }
        Ok(result)
    }

    fn evaluate(&mut self, node: &AstNode, scope: &mut Scope) -> Result<Value, String> {
        use Value::*;

        match &node.node {
            Node::Bool(b) => Ok(Bool(*b)),
            Node::Number(n) => Ok(Number(*n)),
            Node::Str(s) => Ok(StrLiteral(s.clone())),
            Node::Array(elements) => {
                let mut values = Vec::new();
                for node in elements.iter() {
                    match self.evaluate(node, scope)? {
                        Range { min, max } => {
                            for i in min..max {
                                values.push(Number(i as f64))
                            }
                        }
                        value => values.push(value),
                    }
                }
                Ok(Array(Rc::new(values)))
            }
            Node::Range {
                min,
                inclusive,
                max,
            } => {
                let min = self.evaluate(min, scope)?;
                let max = self.evaluate(max, scope)?;
                match (min, max) {
                    (Number(min), Number(max)) => {
                        let min = min as isize;
                        let max = max as isize;
                        let max = if *inclusive { max + 1 } else { max };
                        if min <= max {
                            Ok(Range { min, max })
                        } else {
                            runtime_error!(
                                node.position,
                                format!(
                                    "Invalid range, min should be less than or equal to max - min: {}, max: {}",
                                    min, max
                                ))
                        }
                    }
                    unexpected => runtime_error!(
                        node.position,
                        format!(
                            "Expected numbers for range bounds, found min: {}, max: {}",
                            unexpected.0, unexpected.1
                        )
                    ),
                }
            }
            Node::Index { id, expression } => {
                self.array_index(id, expression, scope, node.position)
            }
            Node::Id(id) => self.get_value(id, scope, node.position),
            Node::Block(block) => self.evaluate_block(&block, scope),
            Node::Function(f) => Ok(Function(f.clone())),
            Node::Call { function, args } => {
                self.call_function(function, args, scope, node.position)
            }
            Node::Assign { id, expression } => {
                let value = self.evaluate(expression, scope)?;
                scope.values.insert(id.clone(), value.clone());
                Ok(value)
            }
            Node::BinaryOp { lhs, op, rhs } => {
                let a = self.evaluate(lhs, scope)?;
                let b = self.evaluate(rhs, scope)?;
                macro_rules! binary_op_error {
                    ($op:ident, $a:ident, $b:ident) => {
                        runtime_error!(
                            node.position,
                            format!(
                                "Unable to perform operation {:?} with lhs: '{}' and rhs: '{}'",
                                op, a, b
                            )
                        )
                    };
                };
                match (&a, &b) {
                    (Number(a), Number(b)) => match op {
                        Op::Add => Ok(Number(a + b)),
                        Op::Subtract => Ok(Number(a - b)),
                        Op::Multiply => Ok(Number(a * b)),
                        Op::Divide => Ok(Number(a / b)),
                        Op::Equal => Ok(Bool(a == b)),
                        Op::NotEqual => Ok(Bool(a != b)),
                        Op::LessThan => Ok(Bool(a < b)),
                        Op::LessThanOrEqual => Ok(Bool(a <= b)),
                        Op::GreaterThan => Ok(Bool(a > b)),
                        Op::GreaterThanOrEqual => Ok(Bool(a >= b)),
                        _ => binary_op_error!(op, a, b),
                    },
                    (Bool(a), Bool(b)) => match op {
                        Op::Equal => Ok(Bool(a == b)),
                        Op::NotEqual => Ok(Bool(a != b)),
                        Op::And => Ok(Bool(*a && *b)),
                        Op::Or => Ok(Bool(*a || *b)),
                        _ => binary_op_error!(op, a, b),
                    },
                    _ => binary_op_error!(op, a, b),
                }
            }
            Node::If {
                condition,
                then_block,
                else_block,
            } => {
                let maybe_bool = self.evaluate(condition, scope)?;
                if let Bool(condition_value) = maybe_bool {
                    if condition_value {
                        self.evaluate_block(then_block, scope)
                    } else if !else_block.is_empty() {
                        self.evaluate_block(else_block, scope)
                    } else {
                        Ok(Value::Empty)
                    }
                } else {
                    runtime_error!(
                        node.position,
                        format!("Expected bool in if statement, found {}", maybe_bool)
                    )
                }
            }
        }
    }

    fn get_value(
        &self,
        id: &Rc<String>,
        scope: &Scope,
        position: Position,
    ) -> Result<Value, String> {
        scope.values.get(id).map_or_else(
            || runtime_error!(position, format!("Value not found: '{}'", id)),
            |v| Ok(v.clone()),
        )
    }

    fn array_index(
        &mut self,
        id: &String,
        expression: &AstNode,
        scope: &mut Scope,
        position: Position,
    ) -> Result<Value, String> {
        use Value::*;

        let index = self.evaluate(expression, scope)?;
        let maybe_array = scope.values.get(id);
        if maybe_array.is_none() {
            return runtime_error!(position, format!("Value not found: '{}'", id));
        }

        if let Some(Array(elements)) = maybe_array {
            match index {
                Number(i) => {
                    let i = i as usize;
                    if i < elements.len() {
                        Ok(elements[i].clone())
                    } else {
                        runtime_error!(
                            position,
                            format!(
                                "Index out of bounds: '{}' has a length of {} but the index is {}",
                                id,
                                elements.len(),
                                i
                            )
                        )
                    }
                }
                Range { min, max } => {
                    let umin = min as usize;
                    let umax = max as usize;
                    if min < 0 || max < 0 {
                        runtime_error!(
                            position,
                            format!(
                                "Indexing with negative indices isn't supported, min: {}, max: {}",
                                min, max
                            )
                        )
                    } else if umin >= elements.len() || umax >= elements.len() {
                        runtime_error!(
                            position,
                            format!(
                                "Index out of bounds: '{}' has a length of {} - min: {}, max: {}",
                                id,
                                elements.len(),
                                min,
                                max
                            )
                        )
                    } else {
                        Ok(Array(Rc::new(
                            elements[umin..umax].iter().cloned().collect::<Vec<_>>(),
                        )))
                    }
                }
                _ => runtime_error!(
                    position,
                    format!(
                        "Indexing is only supported with number values or ranges, found {})",
                        index
                    )
                ),
            }
        } else {
            runtime_error!(
                position,
                format!(
                    "Indexing is only supported for Arrays, found {}",
                    maybe_array.unwrap()
                )
            )
        }
    }

    fn call_function(
        &mut self,
        id: &String,
        args: &Vec<AstNode>,
        scope: &mut Scope,
        position: Position,
    ) -> Result<Value, String> {
        use Value::*;

        let maybe_function_or_error = scope.values.get(id).map(|f| match f {
            Function(f) => Ok(f.clone()),
            unexpected => runtime_error!(
                position,
                format!("Expected function for value {}, found {}", id, unexpected)
            ),
        });
        if let Some(f) = maybe_function_or_error {
            let f = f?;
            let arg_count = f.args.len();
            if args.len() != arg_count {
                return runtime_error!(
                    position,
                    format!(
                        "Incorrect argument count while calling '{}': expected {}, found {} - {:?}",
                        id,
                        arg_count,
                        args.len(),
                        f.args
                    )
                );
            }

            let mut child_scope = Scope::new();

            for (name, arg) in f.args.iter().zip(args.iter()) {
                let arg_value = self.evaluate(arg, scope)?;
                child_scope.values.insert(name.clone(), arg_value);
            }

            return self.evaluate_block(&f.body, &mut child_scope);
        }

        // Builtins, TODO std lib
        let mut arg_values = args.iter().map(|arg| self.evaluate(arg, scope));
        match id.as_str() {
            "assert" => {
                for value in arg_values {
                    match value? {
                        Bool(b) => {
                            if !b {
                                return runtime_error!(position, format!("Assertion failed"));
                            }
                        }
                        _ => {
                            return runtime_error!(
                                position,
                                format!("assert only expects booleans as arguments")
                            )
                        }
                    }
                }
                Ok(Empty)
            }
            "push" => {
                let first_arg_value = match arg_values.next() {
                    Some(arg) => arg,
                    None => {
                        return runtime_error!(
                            position,
                            "Missing array as first argument for push"
                        );
                    }
                };

                match first_arg_value? {
                    Array(array) => {
                        let mut array = array.clone();
                        let array_data = Rc::make_mut(&mut array);
                        for value in arg_values {
                            array_data.push(value?)
                        }
                        Ok(Array(array))
                    }
                    unexpected => {
                        return runtime_error!(
                            position,
                            format!("push is only supported for arrays, found {}", unexpected)
                        )
                    }
                }
            }
            "length" => {
                let first_arg_value = match arg_values.next() {
                    Some(arg) => arg,
                    None => {
                        return runtime_error!(position, "Missing array as argument for length");
                    }
                };
                match first_arg_value? {
                    Array(array) => Ok(Number(array.len() as f64)),
                    Range { min, max } => Ok(Number((max - min) as f64)),
                    unexpected => {
                        return runtime_error!(
                            position,
                            format!(
                                "length is only supported for arrays and ranges, found {}",
                                unexpected
                            )
                        )
                    }
                }
            }
            "print" => {
                for value in arg_values {
                    print!("{} ", value?);
                }
                println!();
                Ok(Empty)
            }
            _ => runtime_error!(
                position,
                format!("Unexpected function name: {}", id.as_str())
            ),
        }
    }
}
