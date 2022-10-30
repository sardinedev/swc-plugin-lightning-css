#![allow(clippy::not_unsafe_ptr_arg_deref)]
mod utils;

use std::collections::HashMap;
use std::fs;

use serde::Deserialize;

use swc_core::common::DUMMY_SP;
use swc_core::ecma::ast::{
    Decl, EmptyStmt, Expr, ImportDefaultSpecifier, ImportSpecifier, JSXAttr, JSXAttrName,
    JSXAttrValue, JSXExpr, KeyValueProp, ModuleDecl, ModuleItem, ObjectLit, Pat, PatOrExpr, Prop,
    PropName, PropOrSpread, Stmt, VarDecl, VarDeclKind, VarDeclarator,
};
use swc_core::ecma::utils::{quote_ident, StmtLike};
use swc_core::plugin::{plugin_transform, proxies::TransformPluginProgramMetadata};
use swc_core::{
    common::util::take::Take,
    ecma::{
        ast::{ImportDecl, Program},
        transforms::testing::test,
        visit::{as_folder, FoldWith, VisitMut, VisitMutWith},
    },
};

#[derive(PartialEq, Debug, Clone, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum CssModuleReference {
    /// A local reference.
    Local {
        /// The local (compiled) name for the reference.
        name: String,
    },
    /// A global reference.
    Global {
        /// The referenced global name.
        name: String,
    },
    /// A reference to an export in a different file.
    Dependency {
        /// The name to reference within the dependency.
        name: String,
        /// The dependency specifier for the referenced file.
        specifier: String,
    },
}

#[derive(PartialEq, Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CssModuleExport {
    /// The local (compiled) name for this export.
    pub name: String,
    /// Other names that are composed by this export.
    pub composes: Vec<CssModuleReference>,
    /// Whether the export is referenced in this file.
    pub is_referenced: bool,
}

/// A map of exported names to values.
pub type CssModuleExports = HashMap<String, CssModuleExport>;

struct TransformVisitor {
    css_module_map: CssModuleExports,
    style_import_name: String,
    should_create_mapped_class_obj: bool,
}

impl VisitMut for TransformVisitor {
    /*
     * Walk the AST and find all CSS Modules import declarations.
     * We mark and remove the module declaration from the AST.
     */
    fn visit_mut_import_decl(&mut self, node: &mut ImportDecl) {
        node.visit_mut_children_with(self);

        if node.specifiers.is_empty() {
            return;
        }

        if node.src.value.ends_with(".module.css") {
            self.css_module_map = get_css_module_mapping(&node.src.value);
            for specifier in &node.specifiers {
                if let ImportSpecifier::Default(ImportDefaultSpecifier { local, span: _ }) =
                    specifier
                {
                    self.style_import_name = local.sym.to_string();
                }
            }

            node.specifiers.take();
        }
    }

    fn visit_mut_jsx_attr(&mut self, attr: &mut JSXAttr) {
        attr.visit_mut_children_with(self);

        // check if the attribute name is "className"
        if let JSXAttrName::Ident(ident) = &attr.name {
            if ident.sym == *"className" {
                if let Some(JSXAttrValue::JSXExprContainer(container)) = attr.value.clone() {
                    if let JSXExpr::Expr(expr) = &container.expr {
                        // Checks if we're just passing a propriety to the className, ie: className={styles.foo}
                        if let Expr::Member(member) = &**expr {
                            if let Expr::Ident(ident) = &*member.obj {
                                if ident.sym == *"styles" {
                                    let prop = &member.prop.as_ident().unwrap().sym.to_string();
                                    // Replace the className with the compiled name
                                    attr.value = Some(JSXAttrValue::Lit(
                                        self.css_module_map[prop].name.clone().into(),
                                    ));
                                }
                            }
                        } else {
                            self.should_create_mapped_class_obj = true;
                        }
                        // Checks if we're passing a string to the className, ie: className={`foo ${styles.foo}`}
                        // if let Expr::Tpl(template) = &**expr {
                        //     for expr in &template.exprs {
                        //         if let Expr::Member(member) = &**expr {
                        //             if let Expr::Ident(obj) = &*member.obj {
                        //                 if obj.sym == *"styles" {
                        //                     if let MemberProp::Ident(ident) = &member.prop {
                        //                         let prop = &ident.sym.to_string();
                        //                         println!("prop: {:?}", prop);
                        //                         println!(
                        //                             "css_module_map: {:?}",
                        //                             self.css_module_map[prop].name
                        //                         );
                        //                     }
                        //                 }
                        //             }
                        //         }
                        //     }
                        // }
                    }
                }
            }
        }
    }

    // fn visit_mut_stmts(&mut self, stmts: &mut Vec<Stmt>) {
    //     self.visit_mut_stmt_like(stmts);
    // }

    // Walk the ASt and finds import declarations that have been marked for removal.
    // We remove top level import declaration from the AST.
    fn visit_mut_module_item(&mut self, node: &mut ModuleItem) {
        node.visit_mut_children_with(self);

        if let ModuleItem::ModuleDecl(ModuleDecl::Import(import)) = node {
            if import.specifiers.is_empty() {
                *node = ModuleItem::Stmt(Stmt::Empty(EmptyStmt { span: DUMMY_SP }));
            }
        }
    }

    fn visit_mut_module_items(&mut self, stmts: &mut Vec<ModuleItem>) {
        self.visit_mut_stmt_like(stmts);
        // stmts.visit_mut_children_with(self);

        // This is also required, because top-level statements are stored in `Vec<ModuleItem>`.
        stmts.retain(|s| {
            // We use `matches` macro as this match is trivial.
            !matches!(s, ModuleItem::Stmt(Stmt::Empty(..)))
        });
    }
}

impl TransformVisitor {
    fn visit_mut_stmt_like<T>(&mut self, stmts: &mut Vec<T>)
    where
        Vec<T>: VisitMutWith<Self>,
        T: StmtLike + VisitMutWith<TransformVisitor>,
    {
        let mut stmts_updated = Vec::with_capacity(stmts.len());

        for mut stmt in stmts.take() {
            stmt.visit_mut_with(self);
            stmts_updated.push(stmt);
        }

        if self.should_create_mapped_class_obj {
            let object_map_name = quote_ident!(DUMMY_SP, "hugo");

            let obj_props = self
                .css_module_map
                .iter()
                .map(|(key, value)| {
                    let key = key.clone();
                    let value = value.name.clone();
                    PropOrSpread::Prop(Box::new(Prop::KeyValue(KeyValueProp {
                        key: PropName::Ident(quote_ident!(DUMMY_SP, key)),
                        value: Box::new(value.into()),
                    })))
                })
                .collect();

            /*
             * Create a new object with the compiled class names, ie:
             * `const styles = { foo: "foo_1", bar: "bar_1" }`
             */
            let style_object = Stmt::Decl(Decl::Var(Box::new(VarDecl {
                span: DUMMY_SP,
                kind: VarDeclKind::Const,
                declare: false,
                decls: vec![VarDeclarator {
                    span: DUMMY_SP,
                    name: quote_ident!(DUMMY_SP, "_styles").into(),
                    init: Some(Box::new(Expr::Object(ObjectLit {
                        span: DUMMY_SP,
                        props: obj_props,
                    }))),
                    definite: false,
                }],
            })));
            stmts_updated.push(T::from_stmt(style_object));
        }

        // let init_var = Stmt::Decl(Decl::Var(Box::new(VarDecl {
        //     span: DUMMY_SP,
        //     kind: VarDeclKind::Const,
        //     declare: false,
        //     decls: vec![VarDeclarator {
        //         span: DUMMY_SP,
        //         name: quote_ident!(DUMMY_SP, "styles").into(),
        //         init: Some(Box::new(Expr::Ident(quote_ident!(
        //             DUMMY_SP,
        //             self.style_import_name.clone()
        //         )))),
        //         definite: false,
        //     }],
        // })));

        // stmts_updated.push(T::from_stmt(init_var));

        *stmts = stmts_updated;
    }
}

/*
 * Takes a relative path to a CSS file and returns an object with the mapping with the scoped class names.
 */
fn get_css_module_mapping(css_file_path: &str) -> CssModuleExports {
    // Replace the .css extension with .json
    let json_file = css_file_path.replace(".css", ".json").replace("./", "");

    let json_file_path = utils::find_file(json_file).unwrap();

    let data = fs::read_to_string(json_file_path).expect("Unable to read file");

    serde_json::from_str::<CssModuleExports>(&data).expect("JSON does not have correct format.")
}

#[plugin_transform]
pub fn process_transform(program: Program, _metadata: TransformPluginProgramMetadata) -> Program {
    program.fold_with(&mut as_folder(TransformVisitor {
        css_module_map: HashMap::new(),
        style_import_name: String::new(),
        should_create_mapped_class_obj: false,
    }))
}

test!(
    Default::default(),
    |_| as_folder(TransformVisitor {
        css_module_map: HashMap::new(),
        style_import_name: String::new(),
        should_create_mapped_class_obj: false,
    }),
    remove_import,
    r#"
    import styles from "./button.module.css";
    export const Button = () => (<button className={styles.button} id="3"/>);
    "#,
    r#"
    export const Button = () => (<button className={styles.button} id="3"/>);
    "#
);
