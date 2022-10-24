#![allow(clippy::not_unsafe_ptr_arg_deref)]
mod utils;

use std::collections::HashMap;
use std::fs;

use serde::Deserialize;

use swc_core::common::DUMMY_SP;
use swc_core::ecma::ast::{
    EmptyStmt, Expr, JSXAttr, JSXAttrName, JSXAttrValue, JSXExpr, ModuleDecl, ModuleItem, Stmt,
};
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
            node.specifiers.take();
        }
    }

    fn visit_mut_jsx_attr(&mut self, attr: &mut JSXAttr) {
        attr.visit_mut_children_with(self);

        // check if the attribute name is "className"
        if let JSXAttrName::Ident(ident) = &attr.name {
            if ident.sym == *"className" {
                if let Some(JSXAttrValue::JSXExprContainer(container)) = &attr.value {
                    if let JSXExpr::Expr(expr) = &container.expr {
                        // Checks if we're just passing a propriety to the className, ie: className={styles.foo}
                        if let Expr::Member(member) = &**expr {
                            if let Expr::Ident(ident) = &*member.obj {
                                if ident.sym == *"styles" {
                                    let obj = &member.prop.as_ident().unwrap().sym.to_string();
                                    println!("CSS Map: {:?}", self.css_module_map[obj].name);
                                    println!("Classes: {:?}", obj);
                                    // Replace the className with the compiled name
                                    attr.value = Some(JSXAttrValue::Lit(
                                        self.css_module_map[obj].name.clone().into(),
                                    ));
                                }
                            }
                        }
                        // Checks if we're passing a string to the className, ie: className={`foo ${styles.foo}`}
                        // if let Expr::Tpl(template) = &**expr {
                        //     for expr in &template.exprs {
                        //         if let Expr::Member(member) = &**expr {
                        //             if let Expr::Ident(ident) = &*member.obj {
                        //                 if ident.sym == *"styles" {
                        //                     let obj =
                        //                         &member.prop.as_ident().unwrap().sym.to_string();
                        //                     println!("Found styles with pros: {:?}", obj);
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
        stmts.visit_mut_children_with(self);

        // This is also required, because top-level statements are stored in `Vec<ModuleItem>`.
        stmts.retain(|s| {
            // We use `matches` macro as this match is trivial.
            !matches!(s, ModuleItem::Stmt(Stmt::Empty(..)))
        });
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
    }))
}

test!(
    Default::default(),
    |_| as_folder(TransformVisitor {
        css_module_map: HashMap::new(),
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
