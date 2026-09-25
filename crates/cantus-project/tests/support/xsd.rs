//! A validator for the part of XML Schema 1.0 that DAWproject's schemas use: global
//! elements and named or anonymous complex types; `sequence`, `choice` and `element`
//! particles with `minOccurs`/`maxOccurs`; `complexContent`/`extension`; attributes of
//! the built-in types the schemas name, of enumerated simple types and of lists; `xs:ID`
//! uniqueness and `xs:IDREF` resolution.
//!
//! Anything in a schema outside that subset is an error here rather than something
//! skipped, so the validator can never pass a document by not understanding its schema.
//! The optional JDK check in `dawproject.rs` runs the full `javax.xml.validation` validator
//! over the same files, to show the two agree.
#![allow(dead_code)]

use roxmltree::{Document, Node};
use std::collections::{BTreeSet, HashMap, HashSet};

const XS: &str = "http://www.w3.org/2001/XMLSchema";
const UNBOUNDED: usize = usize::MAX;

#[derive(Clone, Debug)]
enum TypeRef {
    Named(String),
    Anon(Box<Complex>),
    Builtin(String),
}

#[derive(Clone, Debug)]
enum Kind {
    Elem { name: String, ty: TypeRef },
    Seq(Vec<Particle>),
    Choice(Vec<Particle>),
}

#[derive(Clone, Debug)]
struct Particle {
    kind: Kind,
    min: usize,
    max: usize,
}

#[derive(Clone, Debug)]
enum AttrType {
    Builtin(String),
    Enum(Vec<String>),
    List(Box<AttrType>),
}

#[derive(Clone, Debug)]
struct Attr {
    name: String,
    ty: AttrType,
    required: bool,
}

#[derive(Clone, Debug, Default)]
struct Complex {
    base: Option<String>,
    content: Option<Particle>,
    attrs: Vec<Attr>,
    is_abstract: bool,
}

pub struct Schema {
    elements: HashMap<String, TypeRef>,
    complex: HashMap<String, Complex>,
    simple: HashMap<String, AttrType>,
}

fn xs_children<'a, 'i>(n: Node<'a, 'i>) -> impl Iterator<Item = Node<'a, 'i>> {
    n.children()
        .filter(|c| c.is_element() && c.tag_name().namespace() == Some(XS))
}

fn occurs(n: Node, name: &str) -> usize {
    match n.attribute(name) {
        None => 1,
        Some("unbounded") => UNBOUNDED,
        Some(v) => v.parse().expect("an occurrence count"),
    }
}

fn strip_xs(t: &str) -> String {
    t.strip_prefix("xs:")
        .map(|b| format!("xs:{b}"))
        .unwrap_or_else(|| t.to_string())
}

impl Schema {
    pub fn parse(text: &str) -> Schema {
        let doc = Document::parse(text).expect("the schema parses");
        let mut s = Schema {
            elements: HashMap::new(),
            complex: HashMap::new(),
            simple: HashMap::new(),
        };
        for n in xs_children(doc.root_element()) {
            let name = n.attribute("name").unwrap_or("").to_string();
            match n.tag_name().name() {
                "element" => {
                    let ty = element_type(n);
                    s.elements.insert(name, ty);
                }
                "complexType" => {
                    s.complex.insert(name, complex(n));
                }
                "simpleType" => {
                    s.simple.insert(name, simple(n));
                }
                other => panic!("schema construct <xs:{other}> is not supported"),
            }
        }
        s
    }
}

fn element_type(n: Node) -> TypeRef {
    if let Some(t) = n.attribute("type") {
        if t.starts_with("xs:") {
            TypeRef::Builtin(strip_xs(t))
        } else {
            TypeRef::Named(t.to_string())
        }
    } else if let Some(ct) = xs_children(n).find(|c| c.has_tag_name((XS, "complexType"))) {
        TypeRef::Anon(Box::new(complex(ct)))
    } else {
        TypeRef::Builtin("xs:anyType".into())
    }
}

fn simple(n: Node) -> AttrType {
    let inner = xs_children(n).next().expect("a simple type has a body");
    match inner.tag_name().name() {
        "restriction" => {
            let values: Vec<String> = xs_children(inner)
                .filter(|e| e.has_tag_name((XS, "enumeration")))
                .map(|e| e.attribute("value").unwrap().to_string())
                .collect();
            if values.is_empty() {
                AttrType::Builtin(strip_xs(inner.attribute("base").unwrap()))
            } else {
                AttrType::Enum(values)
            }
        }
        "list" => {
            let item = inner.attribute("itemType").unwrap();
            AttrType::List(Box::new(if item.starts_with("xs:") {
                AttrType::Builtin(strip_xs(item))
            } else {
                AttrType::Builtin(item.to_string())
            }))
        }
        other => panic!("simple type body <xs:{other}> is not supported"),
    }
}

fn attribute(n: Node) -> Attr {
    let ty = match n.attribute("type") {
        Some(t) if t.starts_with("xs:") => AttrType::Builtin(strip_xs(t)),
        Some(t) => AttrType::Builtin(t.to_string()),
        None => simple(
            xs_children(n)
                .find(|c| c.has_tag_name((XS, "simpleType")))
                .expect("an attribute has a type"),
        ),
    };
    Attr {
        name: n.attribute("name").unwrap().to_string(),
        ty,
        required: n.attribute("use") == Some("required"),
    }
}

fn particle(n: Node) -> Particle {
    let (min, max) = (occurs(n, "minOccurs"), occurs(n, "maxOccurs"));
    let kind = match n.tag_name().name() {
        "element" => {
            if let Some(r) = n.attribute("ref") {
                Kind::Elem {
                    name: r.to_string(),
                    ty: TypeRef::Named(format!("@{r}")),
                }
            } else {
                Kind::Elem {
                    name: n.attribute("name").unwrap().to_string(),
                    ty: element_type(n),
                }
            }
        }
        "sequence" => Kind::Seq(xs_children(n).map(particle).collect()),
        "choice" => Kind::Choice(xs_children(n).map(particle).collect()),
        other => panic!("particle <xs:{other}> is not supported"),
    };
    Particle { kind, min, max }
}

fn complex(n: Node) -> Complex {
    let mut c = Complex {
        is_abstract: n.attribute("abstract") == Some("true"),
        ..Default::default()
    };
    for part in xs_children(n) {
        match part.tag_name().name() {
            "sequence" | "choice" => c.content = Some(particle(part)),
            "attribute" => c.attrs.push(attribute(part)),
            "complexContent" => {
                let ext = xs_children(part).next().expect("complexContent has a body");
                assert_eq!(ext.tag_name().name(), "extension", "only extension");
                c.base = Some(ext.attribute("base").unwrap().to_string());
                for x in xs_children(ext) {
                    match x.tag_name().name() {
                        "sequence" | "choice" => c.content = Some(particle(x)),
                        "attribute" => c.attrs.push(attribute(x)),
                        other => panic!("extension body <xs:{other}> is not supported"),
                    }
                }
            }
            other => panic!("complex type body <xs:{other}> is not supported"),
        }
    }
    c
}

/// Everything wrong with a document, empty when it is valid.
pub struct Validation {
    pub errors: Vec<String>,
    ids: HashSet<String>,
    refs: Vec<(String, String)>,
}

impl Schema {
    /// A complex type with its bases folded in: their content first, then its own, and all
    /// their attributes.
    fn resolved(&self, c: &Complex) -> (Vec<Particle>, Vec<Attr>) {
        let (mut content, mut attrs) = match &c.base {
            Some(b) => self.resolved(self.complex.get(b).unwrap_or_else(|| panic!("base {b}"))),
            None => (Vec::new(), Vec::new()),
        };
        content.extend(c.content.clone());
        attrs.extend(c.attrs.iter().cloned());
        (content, attrs)
    }

    pub fn validate(&self, xml: &str) -> Vec<String> {
        let doc = match Document::parse(xml) {
            Ok(d) => d,
            Err(e) => return vec![format!("not well-formed: {e}")],
        };
        let root = doc.root_element();
        let mut v = Validation {
            errors: Vec::new(),
            ids: HashSet::new(),
            refs: Vec::new(),
        };
        match self.elements.get(root.tag_name().name()) {
            Some(t) => self.element(root, t, &mut v, "/"),
            None => v.errors.push(format!(
                "<{}> is not a global element",
                root.tag_name().name()
            )),
        }
        for (r, at) in &v.refs {
            if !v.ids.contains(r) {
                v.errors.push(format!("{at}: IDREF {r:?} names no ID"));
            }
        }
        v.errors
    }

    fn element(&self, n: Node, ty: &TypeRef, v: &mut Validation, path: &str) {
        let path = format!("{path}{}/", n.tag_name().name());
        let complex = match ty {
            TypeRef::Builtin(b) => {
                if n.children().any(|c| c.is_element()) {
                    v.errors.push(format!("{path}: {b} cannot hold elements"));
                }
                if n.attributes().len() > 0 {
                    v.errors.push(format!("{path}: {b} has no attributes"));
                }
                return;
            }
            TypeRef::Anon(c) => (**c).clone(),
            TypeRef::Named(name) => {
                if let Some(global) = name.strip_prefix('@') {
                    let t = self.elements.get(global).expect("a referenced element");
                    return self.element_as(n, t, v, &path);
                }
                self.complex
                    .get(name)
                    .unwrap_or_else(|| panic!("type {name}"))
                    .clone()
            }
        };
        self.check_complex(n, &complex, v, &path);
    }

    fn element_as(&self, n: Node, ty: &TypeRef, v: &mut Validation, path: &str) {
        let complex = match ty {
            TypeRef::Named(name) => self.complex.get(name).unwrap().clone(),
            TypeRef::Anon(c) => (**c).clone(),
            TypeRef::Builtin(_) => return,
        };
        self.check_complex(n, &complex, v, path);
    }

    fn check_complex(&self, n: Node, c: &Complex, v: &mut Validation, path: &str) {
        if c.is_abstract {
            v.errors.push(format!("{path}: the type is abstract"));
        }
        let (content, attrs) = self.resolved(c);
        // Attributes.
        for a in n.attributes() {
            if a.namespace().is_some() {
                continue;
            }
            match attrs.iter().find(|x| x.name == a.name()) {
                None => v
                    .errors
                    .push(format!("{path}: attribute {:?} is not declared", a.name())),
                Some(decl) => {
                    self.check_value(&decl.ty, a.value(), v, &format!("{path}@{}", a.name()))
                }
            }
        }
        for decl in attrs.iter().filter(|d| d.required) {
            if n.attribute(decl.name.as_str()).is_none() {
                v.errors.push(format!(
                    "{path}: required attribute {:?} is missing",
                    decl.name
                ));
            }
        }
        // Children: the names must match the content model…
        let kids: Vec<Node> = n.children().filter(|c| c.is_element()).collect();
        if kids.is_empty() && content.is_empty() {
            return;
        }
        if n.children()
            .any(|c| c.is_text() && !c.text().unwrap_or("").trim().is_empty())
        {
            v.errors
                .push(format!("{path}: text where only elements belong"));
        }
        let model = Particle {
            kind: Kind::Seq(content),
            min: 1,
            max: 1,
        };
        let names: Vec<&str> = kids.iter().map(|k| k.tag_name().name()).collect();
        let ends = repeat(&model, &names, &BTreeSet::from([0]));
        if !ends.contains(&names.len()) {
            v.errors.push(format!(
                "{path}: children {names:?} do not match the content model"
            ));
            return;
        }
        // …and each child its declaration.
        let mut decls = HashMap::new();
        collect(&model, &mut decls);
        for k in kids {
            match decls.get(k.tag_name().name()) {
                Some(t) => self.element(k, t, v, path),
                None => v
                    .errors
                    .push(format!("{path}: <{}> unexpected", k.tag_name().name())),
            }
        }
    }

    fn check_value(&self, ty: &AttrType, value: &str, v: &mut Validation, at: &str) {
        match ty {
            AttrType::Enum(values) => {
                if !values.iter().any(|x| x == value) {
                    v.errors
                        .push(format!("{at}: {value:?} is not one of {values:?}"));
                }
            }
            AttrType::List(item) => {
                for part in value.split_whitespace() {
                    self.check_value(item, part, v, at);
                }
            }
            AttrType::Builtin(b) => match b.as_str() {
                "xs:string" => {}
                "xs:int" => {
                    if value.trim().parse::<i32>().is_err() {
                        v.errors.push(format!("{at}: {value:?} is not an xs:int"));
                    }
                }
                "xs:double" => {
                    let ok = value.trim().parse::<f64>().is_ok()
                        && !value.contains("inf")
                        && !value.contains("NaN");
                    if !ok && !matches!(value, "INF" | "-INF" | "NaN") {
                        v.errors
                            .push(format!("{at}: {value:?} is not an xs:double"));
                    }
                }
                "xs:boolean" => {
                    if !matches!(value, "true" | "false" | "1" | "0") {
                        v.errors
                            .push(format!("{at}: {value:?} is not an xs:boolean"));
                    }
                }
                "xs:ID" => {
                    if !is_ncname(value) || !v.ids.insert(value.to_string()) {
                        v.errors
                            .push(format!("{at}: ID {value:?} is not a unique NCName"));
                    }
                }
                "xs:IDREF" => v.refs.push((value.to_string(), at.to_string())),
                named => match self.simple.get(named) {
                    Some(t) => self.check_value(&t.clone(), value, v, at),
                    None => panic!("attribute type {named} is not supported"),
                },
            },
        }
    }
}

fn is_ncname(s: &str) -> bool {
    let mut c = s.chars();
    matches!(c.next(), Some(f) if f.is_alphabetic() || f == '_')
        && c.all(|ch| ch.is_alphanumeric() || matches!(ch, '_' | '-' | '.'))
}

fn collect(p: &Particle, out: &mut HashMap<String, TypeRef>) {
    match &p.kind {
        Kind::Elem { name, ty } => {
            out.insert(name.clone(), ty.clone());
        }
        Kind::Seq(items) | Kind::Choice(items) => items.iter().for_each(|i| collect(i, out)),
    }
}

/// Where a particle, repeated between its bounds, can end when started at each of `starts`.
fn repeat(p: &Particle, names: &[&str], starts: &BTreeSet<usize>) -> BTreeSet<usize> {
    let mut result = if p.min == 0 {
        starts.clone()
    } else {
        BTreeSet::new()
    };
    let mut current = starts.clone();
    let mut k = 0;
    while k < p.max && k <= names.len() + 1 {
        let next: BTreeSet<usize> = current.iter().flat_map(|s| once(p, names, *s)).collect();
        k += 1;
        if next.is_empty() {
            break;
        }
        if next == current {
            // Matching nothing more: every further repetition ends in the same places.
            result.extend(next);
            break;
        }
        if k >= p.min {
            result.extend(next.iter().copied());
        }
        current = next;
    }
    result
}

/// Where one occurrence of a particle can end when started at `at`.
fn once(p: &Particle, names: &[&str], at: usize) -> BTreeSet<usize> {
    match &p.kind {
        Kind::Elem { name, .. } => {
            if names.get(at) == Some(&name.as_str()) {
                BTreeSet::from([at + 1])
            } else {
                BTreeSet::new()
            }
        }
        Kind::Seq(items) => {
            let mut set = BTreeSet::from([at]);
            for i in items {
                set = repeat(i, names, &set);
                if set.is_empty() {
                    break;
                }
            }
            set
        }
        Kind::Choice(items) => items
            .iter()
            .flat_map(|i| repeat(i, names, &BTreeSet::from([at])))
            .collect(),
    }
}
