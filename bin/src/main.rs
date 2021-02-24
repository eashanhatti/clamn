#![allow(warnings)]

mod lang;
use lang::{
	core::{
        self,
        context::*,
        lang::Note,
        VarInner::*
    },
	surface::{
		Term,
		InnerTerm::*,
		Name,
        QualifiedName
	},
};
use std::{
	collections::HashSet,
	iter::FromIterator,
    io::{Read, Write},
    fs::File,
    path::Path,
    thread
};
mod pass;
use pass::{
    surface_to_core::*,
    text_to_surface::*
};

extern crate pest;
#[macro_use]
extern crate pest_derive;

fn run() {
    let mut s = String::new();
    loop {
        print!("> ");
        std::io::stdout().flush();
        std::io::stdin().read_line(&mut s).unwrap();
        
        if let Some('\n') = s.chars().next_back() { s.pop(); }
        if let Some('\r') = s.chars().next_back() { s.pop(); }

        match &s[0..4] {
            "quit" => break,
            "elab" => {
                let mut file = File::open(&Path::new(&s[5..])).unwrap();
                let mut source = String::new();
                file.read_to_string(&mut source);
                println!("{:?}", source);
                let surface_module = text_to_module(&source);
                s.clear();
                if let Ok(surface_module_ok) = surface_module {
                    // println!("{:?}", surface_module_ok);
                    // let surface_term_type = match infer_type(&surface_term, State::new()) {
                    //     Ok(r#type) => r#type,
                    //     Err(errs) => {
                    //         println!("INFER ERROR\n{:#?}", errs);
                    //         continue;
                    //     }
                    // };
                    let core_module = match elab_toplevel(&surface_module_ok, QualifiedName(Vec::new(), Name(String::from("Main")))) {
                        Ok(module) => module,
                        Err(errs) => {
                            println!("SURFACE ERROR\n{:#?}", errs);
                            continue;
                        }
                    };
                    println!("CORE TERM\n{:?}", core_module);
                    let core_module_type =
                        match core::typing::synth_type(&core_module, Context::new()) {
                            Ok(r#type) => r#type,
                            Err(errs) => { println!("CORE TYPE ERROR\n{:#?}", errs); return; }
                        };
                    println!("CORE TYPECHECK");
                    let now = std::time::Instant::now();
                    match core::typing::check(&core_module, core_module_type, Context::new()) {
                        Ok(()) => println!("NO ERRORS"),
                        Err(errs) => println!("CORE ERROR\n{:#?}", errs)
                    }
                    println!("END CORE TYPECHECK, TIME {:?}", now.elapsed());
                } else {
                    println!("{:#?}", surface_module);
                }
            },
            _ => println!("'{:?}' not a command", &s[0..4])
        }
        s.clear();
    }
}

fn main() {
    run()
}