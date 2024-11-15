//! Fixed-exponent variable-base exponentiation using addition chains.

use num_bigint::BigUint;
use quote::quote;
use syn::Ident;
use crate::addchain::{build_addition_chain, Step};

/// Returns t{n} as an ident.
fn get_temp(n: usize) -> Ident {
    Ident::new(&format!("t{}", n), proc_macro2::Span::call_site())
}

pub(crate) fn generate(
    base: &proc_macro2::TokenStream,
    exponent: BigUint,
) -> proc_macro2::TokenStream {
    let steps = build_addition_chain(exponent);
    // last_usage[i] indicates the last usage of the i-th temporary variable.
    let last_usage = steps
        .iter()
        .enumerate()
        .fold(Vec::new(), |mut acc, (n, step)| {
            acc.push(n);
            match step {
                Step::Double { index } => acc[*index] = n,
                Step::Add { left, right } => {
                    acc[*left] = n;
                    acc[*right] = n;
                }
            }
            acc
        });
    // drops is the ordered sequence of possible drops of the temporary variable
    // the first component is the variable to drop, the second is when to drop it
    let drops: Vec<(usize, usize)> = {
        let mut drops: Vec<(usize, usize)> = last_usage.into_iter().enumerate().collect();
        drops.sort_by_key(|(_i, last_usage)| *last_usage);
        drops
    };

    let mut gen = proc_macro2::TokenStream::new();

    // contains all idents that can be reused
    let mut free_idents: Vec<Ident> = Vec::new();
    // the number of variables that have been used so far.
    let mut n_vars = 0usize;
    // give code for using a free variable, and the corresponding ident
    fn get_free_variable(
        v: &mut Vec<Ident>,
        n_vars: &mut usize,
    ) -> (proc_macro2::TokenStream, Ident) {
        if let Some(last) = v.pop() {
            (quote! { #last }, last)
        } else {
            let ident = get_temp(*n_vars);
            *n_vars += 1;
            (quote! { let mut #ident }, ident)
        }
    }

    // First entry in chain is one, i.e. the base.
    let (start_code, start) = get_free_variable(&mut free_idents, &mut n_vars);
    gen.extend(quote! {
        #start_code = *#base; // the asterisk makes it work
    });

    let mut tmps = vec![start];
    // current location in drops
    let mut drop_index = 0usize;
    for (i, step) in steps.into_iter().enumerate() {
        // allow reuse of everything that will be dropped in this instruction
        while drop_index < drops.len() && drops[drop_index].1 <= i {
            free_idents.push(tmps[drops[drop_index].0].clone());
            drop_index += 1;
        }
        let (out_code, out) = get_free_variable(&mut free_idents, &mut n_vars);

        gen.extend(match step {
            Step::Double { index } => {
                let val = &tmps[index];
                quote! {
                    #out_code = #val.square();
                }
            }
            Step::Add { left, right } => {
                let left = &tmps[left];
                let right = &tmps[right];
                quote! {
                    #out_code = #left * #right;
                }
            }
        });

        tmps.push(out);
    }

    let end = tmps.last().expect("have last");
    gen.extend(quote! {
        #end
    });

    gen
}
