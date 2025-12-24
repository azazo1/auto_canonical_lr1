use std::io::{self, Read};

use bumpalo::Bump;
use clap::Parser;
use lr_analysis::*;

#[derive(clap::Parser)]
struct AppArgs {
    #[clap(short, long)]
    symbol_start: String,
}

fn main() {
    let args = AppArgs::parse();
    let mut inp = String::new();
    io::stdin().read_to_string(&mut inp).unwrap();
    let bump = Bump::new();
    let grammar = Grammar::from_cfg(&inp, args.symbol_start.as_str().into(), &bump)
        .unwrap()
        .augmented();
    for prod in grammar.prods() {
        println!("{:>4} {}", grammar.index_of_prod(prod).unwrap(), prod);
    }
    println!();
    let family = Family::from_grammar(&grammar);
    for (from, is) in family.item_sets().enumerate() {
        println!("I_{from}:");
        for item in is.items() {
            println!("{}", item);
        }
        println!("reduces:");
        for (item, term) in is.reduces() {
            let prod_idx = grammar.index_of_prod(item.prod()).unwrap();
            println!("{term:?} r {prod_idx}");
        }
        println!("gotos:");
        for (tok, to) in family.gotos_of(from).into_iter().flatten() {
            println!("I_{from} -- {tok:?} --> I_{to}");
        }
        println!();
    }
    println!("--- Table ---");
    println!("{}", Table::build_from(&family, &grammar).to_markdown());
}
