use crate::ast::{self, kw};
use crate::parser::{Cursor, Parse, Parser, Peek, Result};

/// The value types for a wasm module.
#[allow(missing_docs)]
#[derive(Debug, PartialEq, Eq, Hash, Copy, Clone)]
pub enum ValType {
    I32,
    I64,
    F32,
    F64,
    Anyref,
    Funcref,
    V128,
    Nullref,
}

impl<'a> Parse<'a> for ValType {
    fn parse(parser: Parser<'a>) -> Result<Self> {
        let mut l = parser.lookahead1();
        if l.peek::<kw::i32>() {
            parser.parse::<kw::i32>()?;
            Ok(ValType::I32)
        } else if l.peek::<kw::i64>() {
            parser.parse::<kw::i64>()?;
            Ok(ValType::I64)
        } else if l.peek::<kw::f32>() {
            parser.parse::<kw::f32>()?;
            Ok(ValType::F32)
        } else if l.peek::<kw::f64>() {
            parser.parse::<kw::f64>()?;
            Ok(ValType::F64)
        } else if l.peek::<kw::anyref>() {
            parser.parse::<kw::anyref>()?;
            Ok(ValType::Anyref)
        } else if l.peek::<kw::funcref>() {
            parser.parse::<kw::funcref>()?;
            Ok(ValType::Funcref)
        } else if l.peek::<kw::anyfunc>() {
            parser.parse::<kw::anyfunc>()?;
            Ok(ValType::Funcref)
        } else if l.peek::<kw::nullref>() {
            parser.parse::<kw::nullref>()?;
            Ok(ValType::Nullref)
        } else if l.peek::<kw::v128>() {
            parser.parse::<kw::v128>()?;
            Ok(ValType::V128)
        } else {
            Err(l.error())
        }
    }
}

/// Type for a `global` in a wasm module
#[derive(Copy, Clone, Debug)]
pub struct GlobalType {
    /// The element type of this `global`
    pub ty: ValType,
    /// Whether or not the global is mutable or not.
    pub mutable: bool,
}

impl<'a> Parse<'a> for GlobalType {
    fn parse(parser: Parser<'a>) -> Result<Self> {
        if parser.peek2::<kw::r#mut>() {
            parser.parens(|p| {
                p.parse::<kw::r#mut>()?;
                Ok(GlobalType {
                    ty: parser.parse()?,
                    mutable: true,
                })
            })
        } else {
            Ok(GlobalType {
                ty: parser.parse()?,
                mutable: false,
            })
        }
    }
}

/// List of different kinds of table types we can have.
///
/// Currently there's only one, a `funcref`.
#[derive(Copy, Clone, Debug)]
pub enum TableElemType {
    /// An element for a table that is a list of functions.
    Funcref,
    /// An element for a table that is a list of `anyref` values.
    Anyref,
    /// An element for a table that is a list of `nullref` values.
    Nullref,
}

impl<'a> Parse<'a> for TableElemType {
    fn parse(parser: Parser<'a>) -> Result<Self> {
        // legacy support for `anyfunc`
        if parser.peek::<kw::anyfunc>() {
            parser.parse::<kw::anyfunc>()?;
            return Ok(TableElemType::Funcref);
        }
        let mut l = parser.lookahead1();
        if l.peek::<kw::funcref>() {
            parser.parse::<kw::funcref>()?;
            Ok(TableElemType::Funcref)
        } else if l.peek::<kw::anyref>() {
            parser.parse::<kw::anyref>()?;
            Ok(TableElemType::Anyref)
        } else if l.peek::<kw::nullref>() {
            parser.parse::<kw::nullref>()?;
            Ok(TableElemType::Nullref)
        } else {
            Err(l.error())
        }
    }
}

impl Peek for TableElemType {
    fn peek(cursor: Cursor<'_>) -> bool {
        kw::funcref::peek(cursor)
            || kw::anyref::peek(cursor)
            || /* legacy */ kw::anyfunc::peek(cursor)
    }
    fn display() -> &'static str {
        "table element type"
    }
}

/// Min/max limits used for tables/memories.
#[derive(Copy, Clone, Debug)]
pub struct Limits {
    /// The minimum number of units for this type.
    pub min: u32,
    /// An optional maximum number of units for this type.
    pub max: Option<u32>,
}

impl<'a> Parse<'a> for Limits {
    fn parse(parser: Parser<'a>) -> Result<Self> {
        let min = parser.parse()?;
        let max = if parser.peek::<u32>() {
            Some(parser.parse()?)
        } else {
            None
        };
        Ok(Limits { min, max })
    }
}

/// Configuration for a table of a wasm mdoule
#[derive(Copy, Clone, Debug)]
pub struct TableType {
    /// Limits on the element sizes of this table
    pub limits: Limits,
    /// The type of element stored in this table
    pub elem: TableElemType,
}

impl<'a> Parse<'a> for TableType {
    fn parse(parser: Parser<'a>) -> Result<Self> {
        Ok(TableType {
            limits: parser.parse()?,
            elem: parser.parse()?,
        })
    }
}

/// Configuration for a memory of a wasm module
#[derive(Copy, Clone, Debug)]
pub struct MemoryType {
    /// Limits on the page sizes of this memory
    pub limits: Limits,
    /// Whether or not this is a shared (atomic) memory type
    pub shared: bool,
}

impl<'a> Parse<'a> for MemoryType {
    fn parse(parser: Parser<'a>) -> Result<Self> {
        let limits: Limits = parser.parse()?;
        let shared = parser.parse::<Option<kw::shared>>()?.is_some();
        Ok(MemoryType { limits, shared })
    }
}

/// A function type with parameters and results.
#[derive(Clone, Debug)]
pub struct FunctionType<'a> {
    /// The parameters of a function, optionally each having an identifier for
    /// name resolution and a name for the custom `name` section.
    pub params: Vec<(Option<ast::Id<'a>>, Option<ast::NameAnnotation<'a>>, ValType)>,
    /// The results types of a function.
    pub results: Vec<ValType>,
}

impl<'a> FunctionType<'a> {
    fn finish_parse(&mut self, allow_names: bool, parser: Parser<'a>) -> Result<()> {
        while parser.peek2::<kw::param>() || parser.peek2::<kw::result>() {
            parser.parens(|p| {
                let mut l = p.lookahead1();
                if l.peek::<kw::param>() {
                    if self.results.len() > 0 {
                        return Err(p.error(
                            "result before parameter (or unexpected token): \
                             cannot list params after results",
                        ));
                    }
                    p.parse::<kw::param>()?;
                    if p.is_empty() {
                        return Ok(());
                    }
                    let (id, name) = if allow_names {
                        (p.parse::<Option<_>>()?, p.parse::<Option<_>>()?)
                    } else {
                        (None, None)
                    };
                    let parse_more = id.is_none() && name.is_none();
                    let ty = p.parse()?;
                    self.params.push((id, name, ty));
                    while parse_more && !p.is_empty() {
                        self.params.push((None, None, p.parse()?));
                    }
                } else if l.peek::<kw::result>() {
                    p.parse::<kw::result>()?;
                    while !p.is_empty() {
                        self.results.push(p.parse()?);
                    }
                } else {
                    return Err(l.error());
                }
                Ok(())
            })?;
        }
        Ok(())
    }
}

impl<'a> Parse<'a> for FunctionType<'a> {
    fn parse(parser: Parser<'a>) -> Result<Self> {
        parser.parse::<kw::func>()?;
        let mut ret = FunctionType {
            params: Vec::new(),
            results: Vec::new(),
        };
        ret.finish_parse(true, parser)?;
        Ok(ret)
    }
}

/// A type declaration in a module
#[derive(Debug)]
pub struct Type<'a> {
    /// An optional identifer to refer to this `type` by as part of name
    /// resolution.
    pub id: Option<ast::Id<'a>>,
    /// The type that we're declaring.
    pub func: FunctionType<'a>,
}

impl<'a> Parse<'a> for Type<'a> {
    fn parse(parser: Parser<'a>) -> Result<Self> {
        parser.parse::<kw::r#type>()?;
        let id = parser.parse()?;
        let func = parser.parens(FunctionType::parse)?;
        Ok(Type { id, func })
    }
}

/// A reference to a type defined in this module.
///
/// This is a pretty tricky type used in a lot of places and is somewhat subtly
/// handled as well. In general `(type)` or `(param)` annotations are parsed as
/// this.
#[derive(Clone, Debug)]
pub struct TypeUse<'a> {
    /// The span of the index specifier, if it was found
    pub index_span: Option<ast::Span>,
    /// The type that we're referencing, if it was present.
    pub index: Option<ast::Index<'a>>,
    /// The inline function type defined. If nothing was defined inline this is
    /// empty.
    pub ty: ast::FunctionType<'a>,
}

impl<'a> TypeUse<'a> {
    /// Parse a `TypeUse`, but don't allow any names of `param` tokens.
    pub fn parse_no_names(parser: Parser<'a>) -> Result<Self> {
        TypeUse::parse_allow_names(parser, false)
    }

    fn parse_allow_names(parser: Parser<'a>, allow_names: bool) -> Result<Self> {
        let index = if parser.peek2::<kw::r#type>() {
            Some(parser.parens(|parser| {
                parser.parse::<kw::r#type>()?;
                Ok((parser.cur_span(), parser.parse()?))
            })?)
        } else {
            None
        };
        let (index_span, index) = match index {
            Some((a, b)) => (Some(a), Some(b)),
            None => (None, None),
        };
        let mut ty = FunctionType {
            params: Vec::new(),
            results: Vec::new(),
        };
        if parser.peek2::<kw::param>() || parser.peek2::<kw::result>() {
            ty.finish_parse(allow_names, parser)?;
        }

        Ok(TypeUse {
            index,
            index_span,
            ty,
        })
    }
}

impl<'a> Parse<'a> for TypeUse<'a> {
    fn parse(parser: Parser<'a>) -> Result<Self> {
        TypeUse::parse_allow_names(parser, true)
    }
}
