#![allow(warnings)]

use super::{
	lang::{
		*,
		InnerTerm::*,
		List::*
	},
	eval::*,
	context::*
};
use std::{
	default::*,
	cmp::max,
	collections::{
		HashMap,
		HashSet
	},
	fmt::Debug,
	hash::Hash
};

// for the `Expected...` errors, imagine a TypeType `U` for each error, the error
// can then be thought of as `MismatchedTypes { exp_type: U, giv_type: ... }
#[derive(Debug)]
pub enum InnerError {
    MismatchedTypes { exp_type: Term, giv_type: Term },
    NonexistentVar { index: usize },
    ExpectedOfTypeType { min_level: usize, giv_type: Term },
    ExpectedOfFunctionType { giv_type: Term },
    ExpectedOfPairType { giv_type: Term },
    ExpectedOfEnumType { giv_type: Term },
    ExpectedOfFoldType { giv_type: Term },
    ExpectedOfCapturesListType { min_level: usize, giv_type: Term },
    ExpectedOfUnitType { giv_type: Term },
    MismatchedUsage { var_index: usize, exp_usage: (usize, usize), giv_usage: (usize, usize) },
    UniqueTypeInSharedType,
    ExpectedOfSharedTypeType,
    UnmentionedFreeVars { caps_list: Vec<Term>, unmentioned_vars: Vec<Term> }
}

#[derive(Debug)]
pub struct Error<'a> {
    term: &'a Term,
    context: Context,
    error: InnerError
}

impl<'a> Error<'a> {
    pub fn new(term: &'a Term, context: Context, error: InnerError) -> Error<'a> {
        Error {
            term,
            context,
            error
        }
    }
}

pub type CheckResult<'a, U> = Result<U, Vec<Error<'a>>>;

pub fn push_check<'a, U>(errors: &mut Vec<Error<'a>>, this_check: CheckResult<'a, U>) { // appends errors to an error list, if there are any
	match this_check {
		Ok(_) => (),
		Err(errs) => {
			for err in errs {
				errors.push(err);
			}
		}
	}
}

// checks if two terms are equal, disregarding type ann
pub fn is_terms_eq(type1: &Term, type2: &Term) -> bool {
	match (&(*type1.0), &(*type2.0)) {
		(&TypeTypeIntro(level1, usage1), &TypeTypeIntro(level2, usage2)) =>
			level1 == level2 && usage1 == usage2,
		(&Var(index1), &Var(index2)) =>
			index1 == index2,
		(&Rec(ref inner_term1), &Rec(ref inner_term2)) =>
			is_terms_eq(inner_term1, inner_term2),
		(&FunctionTypeIntro(ref caps_list1, ref in_type1, ref out_type1), &FunctionTypeIntro(ref caps_list2, ref in_type2, ref out_type2)) =>
			is_terms_eq(caps_list1, caps_list2) && is_terms_eq(in_type1, in_type2) && is_terms_eq(out_type1, out_type2),
		(&FunctionIntro(ref body1), &FunctionIntro(ref body2)) =>
			is_terms_eq(body1, body2),
		(&FunctionElim(ref abs1, ref arg1), &FunctionElim(ref abs2, ref arg2)) =>
			is_terms_eq(abs1, abs2) && is_terms_eq(arg1, arg2),
		(&PairTypeIntro(ref fst_type1, ref snd_type1), &PairTypeIntro(ref fst_type2, ref snd_type2)) =>
			is_terms_eq(fst_type1, snd_type1) && is_terms_eq(fst_type2, snd_type2),
		(&PairIntro(ref fst1, ref snd1), &PairIntro(ref fst2, ref snd2)) =>
			is_terms_eq(fst1, fst2) && is_terms_eq(snd1, snd2),
		(&PairElim(ref discrim1, ref body1), &PairElim(ref discrim2, ref body2)) =>
			is_terms_eq(discrim1, discrim2) && is_terms_eq(body1, body2),
		(&DoubTypeIntro, &DoubTypeIntro) => true,
		(&DoubIntro(ref label1), &DoubIntro(ref label2)) =>
			label1 == label2,
		(&DoubElim(ref discrim1, ref branch11, ref branch21), &DoubElim(ref discrim2, ref branch12, ref branch22)) =>
			is_terms_eq(discrim1, discrim2) && is_terms_eq(branch11, branch12) && is_terms_eq(branch21, branch22),
		(&FoldTypeIntro(ref inner_type1), &FoldTypeIntro(ref inner_type2)) =>
			is_terms_eq(inner_type1, inner_type2),
		(&FoldIntro(ref inner_term1), &FoldIntro(ref inner_term2)) =>
			is_terms_eq(inner_term1, inner_term2),
		(&FoldElim(ref inner_term1), &FoldElim(ref inner_term2)) =>
			is_terms_eq(inner_term1, inner_term2),
		_ => false
	}
}

pub fn count_uses(term: &Term, target_index: usize) -> (usize, usize) {
	fn collapse(intervals: Vec<(usize, usize)>) -> (usize, usize) {
		let mut min = std::usize::MAX;
		let mut max = 0;
		for (b1, b2) in intervals {
			if b1 < min { min = b1 }
			if b2 > max { max = b2 }
		}
		(min, max)
	}

	fn sum(intervals: Vec<(usize, usize)>) -> (usize, usize) {
		let mut min = 0;
		let mut max = 0;
		for (b1, b2) in intervals {
			min += b1;
			max += b2;
		}
		(min, max)
	}

	fn mul(intervals: Vec<(usize, usize)>) -> (usize, usize) {
		let mut min = 0;
		let mut max = 0;
		for (b1, b2) in intervals {
			min *= b1;
			max *= b2;
		}
		(min, max)
	}

	fn singleton(bound: usize) -> (usize, usize) { (bound, bound) }

	sum(vec![
		match *term.0 {
			TypeTypeIntro(level, usage) => singleton(0),
			Var(index) => if index == target_index { singleton(1) } else { singleton(0) },
			Rec(ref inner_term) => count_uses(inner_term, target_index + 1),
			FunctionTypeIntro(ref caps_list, ref in_type, ref out_type) =>
				sum(vec![
					count_uses(caps_list, target_index),
					mul(vec![
						count_uses(in_type, target_index),
						count_uses(out_type, 0)
					]),
					count_uses(out_type, target_index + 1)
				]),
			FunctionIntro(ref body) => count_uses(body, target_index + 1),
			FunctionElim(ref abs, ref arg) => unimplemented!(),
			PairTypeIntro(ref fst_type, ref snd_type) =>
				sum(vec![count_uses(fst_type, target_index + 1), count_uses(snd_type, target_index + 1)]),
			PairIntro(ref fst, ref snd) =>
				sum(vec![count_uses(fst, target_index), count_uses(snd, target_index)]),
			PairElim(ref discrim, ref body) =>
				sum(vec![count_uses(discrim, target_index), count_uses(body, target_index + 2)]),
			VoidTypeIntro => singleton(0),
			UnitTypeIntro => singleton(0),
			UnitIntro => singleton(0),
			DoubTypeIntro => singleton(0),
			DoubIntro(_) => singleton(0),
			DoubElim(ref discrim, ref branch1, ref branch2) =>
				sum(vec![
					count_uses(branch1, target_index),
					count_uses(branch2, target_index),
					count_uses(discrim, target_index)
				]),
			FoldTypeIntro(ref inner_type) => count_uses(inner_type, target_index),
			FoldIntro(ref inner_term) => count_uses(inner_term, target_index),
			FoldElim(ref folded_term) => count_uses(folded_term, target_index),
			CapturesListTypeIntro(_) => singleton(0),
			CapturesListIntro(ref list) =>
				match list {
					Cons(ref head, ref tail) =>
						sum(vec![
							count_uses(head, target_index),
							count_uses(tail, target_index)
						]),
					Nil => singleton(0)
				}
		},
		count_uses(&term.type_raw(), target_index)])
}

// `term` should be checked before this is called
// should make this more robust in the future
fn get_free_vars(term: &Term) -> HashSet<(usize, Term)> {
	type Set = HashSet<(usize, Term)>;

	fn inner(term: &Term, bounds: HashSet<usize>) -> Set {
		fn collapse(sets: Vec<Set>) -> Set {
			let mut new_set: Set = HashSet::new();
			for set in sets {
				new_set = new_set.r#union(&set).cloned().collect::<Set>();
			}
			new_set
		}

		fn inc(set: HashSet<usize>) -> HashSet<usize> {
			let mut tmp = set.into_iter().map(|i| i + 1).collect::<HashSet<usize>>();
			tmp.insert(0);
			tmp
		}

		let type_ann_free_vars = inner(&term.type_raw(), bounds.clone());
		collapse(vec![
			match *term.0 {
				TypeTypeIntro(_, _) => HashSet::new(),
				Var(index) => panic!(),
				Rec(ref inner_term) => inner(inner_term, inc(bounds.clone())),
				FunctionTypeIntro(ref caps_list, ref in_type, ref out_type) =>
					collapse(vec![
						inner(caps_list, bounds.clone()),
						inner(in_type, bounds.clone()),
						inner(out_type, inc(bounds))
					]),
				FunctionIntro(ref body) => inner(body, inc(bounds)),
				FunctionElim(ref abs, ref arg) =>
					collapse(vec![
						inner(abs, bounds.clone()),
						inner(arg, bounds)
					]),
				PairTypeIntro(ref fst_type, ref snd_type) =>
					collapse(vec![
						inner(fst_type, inc(bounds.clone())),
						inner(snd_type, inc(bounds))
					]),
				PairIntro(ref fst, ref snd) =>
					collapse(vec![
						inner(fst, bounds.clone()),
						inner(snd, bounds)
					]),
				PairElim(ref discrim, ref body) =>
					collapse(vec![
						inner(discrim, bounds.clone()),
						inner(body, inc(bounds))
					]),
				VoidTypeIntro => HashSet::new(),
				UnitTypeIntro => HashSet::new(),
				UnitIntro => HashSet::new(),
				DoubTypeIntro => HashSet::new(),
				DoubIntro(_) => HashSet::new(),
				DoubElim(ref discrim, ref branch1, ref branch2) =>
					collapse(vec![
						inner(branch1, bounds.clone()),
						inner(branch2, bounds.clone()),
						inner(discrim, bounds)
					]),
				FoldTypeIntro(ref inner_type) => inner(inner_type, bounds.clone()),
				FoldIntro(ref inner_term) => inner(inner_term, bounds.clone()),
				FoldElim(ref folded_term) => inner(folded_term, bounds),
				CapturesListTypeIntro(_) => HashSet::new(),
				CapturesListIntro(ref list) =>
					match list {
						Cons(ref head, ref tail) =>
							collapse(vec![
								inner(head, bounds.clone()),
								inner(tail, bounds)
							]),
						Nil => HashSet::new()
					}
			},
			type_ann_free_vars])
	}

	inner(term, HashSet::new())
}

pub fn wrap_checks<'a>(errors: Vec<Error<'a>>) -> CheckResult<'a, ()> {
	if errors.len() > 0 {
		Err(errors)
	} else {
		Ok(())
	}
}

pub fn check_usage<'a>(binder: &'a Term, term_type: Term, body: &'a Term, target_index: usize, context: Context) -> CheckResult<'a, ()> {
	use InnerError::*;

	match term_type.usage(context.clone()) {
		Shared => Ok(()),
		Unique =>
			if count_uses(body, 0) == (1, 1) {
				Ok(())
			} else {
				Err(vec![Error::new(binder, context, MismatchedUsage { var_index: target_index, exp_usage: (1, 1), giv_usage: count_uses(body, 0) })])
			}
	}
}

// r#type should be checked with `check` before being checked with this
pub fn check_type<'a>(r#type: &'a Term, context: Context) -> CheckResult<'a, ()> {
	// panic!("`check_type` is not finished");

	// fn check_type_aux<'a>(r#type: &'a Term, context: Context, exp_shared: bool) -> CheckResult<'a, ()> {
	// 	let exp_usage =
	// 		match r#type.usage(context.clone()) {
	// 			Shared => true,
	// 			Unique => false
	// 		};

	// 	match *r#type.0 {
	// 		Ann(ref annd_term, ref type_ann) => {
	// 			let mut errors = Vec::new();
	// 			push_check(&mut errors, check_type_aux(type_ann, context.clone()));
	// 			push_check(&mut errors, check_type_aux(annd_term, context, exp_usage));
	// 			wrap_checks(errors)
	// 		},
	// 		// Rec(ref inner_term) => check_type_aux(inner_term, context, exp_usage),
	// 		FunctionTypeIntro(ref in_type, ref out_type) => {
	// 			let mut errors = Vec::new();
	// 			push_check(&mut errors, check_type_aux(in_type, context.clone()));
	// 			push_check(&mut errors, check_type_aux(out_type, context));
	// 			wrap_checks(errors)
	// 		},
	// 		FunctionIntro(ref body) => check_type_aux(body, context, exp_usage),
	// 		// FunctionElim(ref abs, ref arg) => {
	// 		// 	let errors = Vec::new();
	// 		// 	push_check(&mut errors, check_type_aux(abs,))
	// 		// }
	// 		PairTypeIntro(ref fst_type, ref snd_type) => {
	// 			let mut errors = Vec::new();
	// 			push_check(&mut errors, check_type_aux(fst_type, context.clone(), exp_usage));
	// 			push_check(&mut errors, check_type_aux(snd_type, context, exp_usage));
	// 			wrap_checks(errors)
	// 		},
	// 		FoldTypeIntro(ref inner_type) => check_type_aux(inner_type, context, exp_usage),
	// 		_ =>
	// 			match (r#type.usage(), exp_shared) {
	// 				(Shared, true) => Ok(()),
	// 				(Unique, true) => Err(vec![Error::new(r#type, ExpectedSharedTypeType)]),
	// 				(Shared, false) => Ok(()),
	// 				(Unique, false) => Ok(())
	// 			}
	// 	}
	// }

	// let mut errors = Vec::new();
	// push_check(&mut errors, check_type_aux(r#type, context, should_expect_shared(r#type)));
	// wrap_checks(errors)
	Ok(()) // until i figure out how this is supposed to work
}

// exp_type should always be checked and in normal form
pub fn check<'a>(term: &'a Term, exp_type: Term, context: Context) -> CheckResult<'a, ()> {
	use InnerError::*;
	
	let type_ann = term.r#type(context.clone())?;
	if !is_terms_eq(&type_ann, &exp_type) {
		return
			Err(vec![Error::new(
				term,
				context,
				MismatchedTypes { exp_type: exp_type.clone(), giv_type: type_ann.clone() })]);
	}

	match *term.0 {
		// Ann(ref annd_term, ref type_ann) => {
		// 	let mut errors = Vec::new();
			
		// 	let type_ann_type = type_ann.r#type(context.clone())?;
		// 	let normal_type_ann = normalize(type_ann.clone(), Context::new());

		// 	push_check(&mut errors, check(type_ann, type_ann_type, context.clone()));
		// 	push_check(&mut errors, check_type(type_ann, context.clone()));
		// 	push_check(&mut errors, check(annd_term, normal_type_ann.clone(), context.clone()));
		// 	push_check(
		// 		&mut errors,
		// 		if is_terms_eq(&normal_type_ann, &type_ann) {
		// 			Ok(())
		// 		} else {
		// 			Err(vec![Error::new(term, context, MismatchedTypes { exp_type: type_ann.clone(), giv_type: normal_type_ann.clone() })])
		// 		});

		// 	wrap_checks(errors)
		// },
		TypeTypeIntro(level, usage) =>
			match *(type_ann.clone()).0 {
				TypeTypeIntro(type_ann_level, type_ann_usage) =>
					if type_ann_level > level {
						Ok(())
					} else {
						Err(vec![Error::new(term, context, ExpectedOfTypeType { min_level: level + 1, giv_type: type_ann })])
					}
				_ => Err(vec![Error::new(term, context, ExpectedOfTypeType { min_level: level + 1, giv_type: type_ann })])
			},
		Var(index) =>
			match context.find_dec(index) {
				Some(var_type) =>
					if is_terms_eq(&var_type, &type_ann) {
						Ok(())
					} else {
						Err(vec![Error::new(term, context, MismatchedTypes { exp_type: type_ann, giv_type: var_type })])
					}
				None => Err(vec![Error::new(term, context, NonexistentVar { index })])
			},
		Rec(ref inner_term) => {
			let mut errors = Vec::new();

			let new_context = context.clone().inc_and_shift(1);
			let inner_term_type = inner_term.r#type(new_context.clone())?; // for now, all recursive functions must be type annotated

			push_check(
				&mut errors,
				check(inner_term, inner_term_type.clone(), new_context.insert_dec(0, inner_term_type.clone())));
			push_check(&mut errors, check_usage(&term, inner_term_type, inner_term, 0, context.clone()));

			wrap_checks(errors)

		},
		FunctionTypeIntro(ref caps_list, ref in_type, ref out_type) => {
			let mut errors = Vec::new();

			let out_type_context = context.clone().inc_and_shift(1).insert_dec(0, in_type.clone());

			let caps_list_type = caps_list.r#type(context.clone())?;
			let in_type_type = in_type.r#type(context.clone())?;
			let out_type_type = out_type.r#type(out_type_context.clone())?;
			push_check(
				&mut errors,
				check(caps_list, caps_list_type.clone(), context.clone()));
			push_check(
				&mut errors,
				check(in_type, in_type_type.clone(), context.clone()));
			push_check(
				&mut errors,
				check(out_type, out_type_type.clone(), out_type_context));

			push_check(&mut errors, check_usage(&term, in_type.clone(), &out_type, 0, context.clone().inc_and_shift(1).clone()));

			match (*(caps_list_type.clone()).0, *(in_type_type.clone()).0, *(out_type_type.clone()).0) {
				(CapturesListTypeIntro(caps_list_level), TypeTypeIntro(in_level, in_usage), TypeTypeIntro(out_level, out_usage)) => {
					let giv_max = max(caps_list_level, max(in_level, out_level));
					if let TypeTypeIntro(max_level, fn_usage) = *type_ann.clone().0 {
						if giv_max != max_level {
							errors.push(Error::new(
								term,
								context,
								MismatchedTypes {
									exp_type: Term(Box::new(TypeTypeIntro(max_level, fn_usage)), None),
									giv_type: Term(Box::new(TypeTypeIntro(giv_max, fn_usage)), None)
								}));
						}
					} else {
						errors.push(Error::new(term, context, ExpectedOfTypeType { min_level: giv_max, giv_type: type_ann }))
					}
				},
				(_, _, TypeTypeIntro(level, _)) => {
					errors.push(Error::new(in_type, context.clone(), ExpectedOfTypeType { min_level: level, giv_type: in_type_type }));
					errors.push(Error::new(caps_list, context, ExpectedOfCapturesListType { min_level: level, giv_type: caps_list_type }));
				}
				(_, TypeTypeIntro(level, _), _) => {
					errors.push(Error::new(out_type, context.clone(), ExpectedOfTypeType { min_level: level, giv_type: out_type_type }));
					errors.push(Error::new(caps_list, context, ExpectedOfCapturesListType { min_level: level, giv_type: caps_list_type }));
				}
				(_, _, _) =>  {
					errors.push(Error::new(in_type, context.clone(), ExpectedOfTypeType { min_level: 0, giv_type: in_type_type }));
					errors.push(Error::new(out_type, context.clone(), ExpectedOfTypeType { min_level: 0, giv_type: out_type_type }));
					errors.push(Error::new(caps_list, context, ExpectedOfCapturesListType { min_level: 0, giv_type: caps_list_type }));
				},
				(CapturesListTypeIntro(level1), _, TypeTypeIntro(level2, _)) =>
					errors.push(Error::new(in_type, context, ExpectedOfTypeType { min_level: max(level1, level2), giv_type: in_type_type })),
				(CapturesListTypeIntro(level1), TypeTypeIntro(level2, _), _) =>
					errors.push(Error::new(out_type, context, ExpectedOfTypeType { min_level: max(level1, level2), giv_type: out_type_type })),
				(CapturesListTypeIntro(level1), _, _) =>  {
					errors.push(Error::new(in_type, context.clone(), ExpectedOfTypeType { min_level: level1, giv_type: in_type_type }));
					errors.push(Error::new(out_type, context, ExpectedOfTypeType { min_level: level1, giv_type: out_type_type }));
				}
			}

			wrap_checks(errors)
		},
		FunctionIntro(ref body) => {
			let mut errors = Vec::new();

			match *type_ann.0 {
				FunctionTypeIntro(caps_list, in_type, out_type) => {
					let body_context = context.clone().inc_and_shift(1).insert_dec(0, shift(in_type.clone(), HashSet::new(), 1));
					push_check(&mut errors, check(body, out_type, body_context));
					push_check(&mut errors, check_usage(term, in_type, body, 0, context.clone().inc_and_shift(1)));

					fn flatten_caps_list(caps_list: &Term) -> Vec<Term> {
						fn inner(caps_list: &Term, acc: &mut Vec<Term>) {
							match *caps_list.0 {
								CapturesListIntro(ref list) => 
									match list {
										Cons(ref head, ref tail) => {
											acc.push(head.clone());
											inner(tail, acc);
										},
										Nil => ()
									},
								_ => ()
							}
						}

						let mut vec = Vec::new();
						inner(caps_list, &mut vec);
						vec
					}
					let capd_var_types = flatten_caps_list(&caps_list);
					let free_var_types = get_free_vars(body).into_iter().map(|(_, t)| t).collect::<HashSet<Term>>();

					// find the captured vars that are not mentioned in the captures list, if any
					let mut leftover_vars = Vec::new();
					let mut used = HashSet::new();
					'top: for free_var_type in free_var_types {
						for (i, capd_var_type) in capd_var_types.iter().enumerate() {
							if !used.contains(&i) {
								if *capd_var_type == free_var_type {
									used.insert(i);
									continue 'top;
								}
							}
						}
						leftover_vars.push(free_var_type);
					}

					if leftover_vars.len() > 0 {
						errors.push(Error::new(term, context, UnmentionedFreeVars { caps_list: capd_var_types, unmentioned_vars: leftover_vars }))
					}
				},
				_ => errors.push(Error::new(term, context, ExpectedOfFunctionType { giv_type: type_ann }))
			}

			wrap_checks(errors)
		}
		FunctionElim(ref abs, ref arg) => {
			let mut errors = Vec::new();

			let abs_type = abs.r#type(context.clone())?;
			push_check(&mut errors, check(abs, abs_type.clone(), context.clone()));


			match *abs_type.0 {
				FunctionTypeIntro(caps_list, in_type, out_type) => push_check(&mut errors, check(arg, in_type, context.clone())),
				_ => errors.push(Error::new(term, context, ExpectedOfFunctionType { giv_type: abs_type }))
			}

			wrap_checks(errors)
		},
		PairTypeIntro(ref fst_type, ref snd_type) => {
			let mut errors = Vec::new();

			let fst_type_type = fst_type.r#type(context.clone())?;
			push_check(
				&mut errors,
				check(fst_type, fst_type_type.clone(), context.clone().inc_and_shift(2).insert_dec(1, snd_type.clone())));

			let snd_type_type = snd_type.r#type(context.clone())?;
			push_check(
				&mut errors,
				check(snd_type, snd_type_type.clone(), context.clone().inc_and_shift(2).insert_dec(0, fst_type.clone())));

			push_check(&mut errors, check_usage(&term, fst_type.clone(), snd_type, 0, context.clone()));
			push_check(&mut errors, check_usage(&term, snd_type.clone(), fst_type, 1, context.clone()));

			match (*(fst_type_type.clone()).0, *(snd_type_type.clone()).0) {
				(TypeTypeIntro(fst_level, fst_usage), TypeTypeIntro(snd_level, snd_usage)) =>
					if let TypeTypeIntro(max_level, pair_usage) = *type_ann.clone().0 {
						if max(fst_level, snd_level) != max_level {
							errors.push(Error::new(
								term,
								context,
								MismatchedTypes {
									exp_type: Term(Box::new(TypeTypeIntro(max_level, pair_usage)), None),
									giv_type: Term(Box::new(TypeTypeIntro(max(fst_level, snd_level), pair_usage)), None)
								}));
						}
					} else {
						errors.push(Error::new(term, context, ExpectedOfTypeType { min_level: max(fst_level, snd_level), giv_type: type_ann }))
					},
				(_, TypeTypeIntro(level, _)) => errors.push(Error::new(fst_type, context, ExpectedOfTypeType { min_level: level, giv_type: fst_type_type })),
				(TypeTypeIntro(level, _), _) => errors.push(Error::new(snd_type, context, ExpectedOfTypeType { min_level: level, giv_type: snd_type_type })),
				(_, _) =>  {
					errors.push(Error::new(fst_type, context.clone(), ExpectedOfTypeType { min_level: 0, giv_type: fst_type_type }));
					errors.push(Error::new(snd_type, context, ExpectedOfTypeType { min_level: 0, giv_type: snd_type_type }));
				}
			}

			wrap_checks(errors)
		},
		PairIntro(ref fst, ref snd) => {
			match *type_ann.0 {
				PairTypeIntro(fst_type, snd_type) => {
					let mut errors = Vec::new();
					let fst_check = check(fst, fst_type.clone(), context.clone().inc_and_shift(2).insert_dec(1, snd_type.clone()));
					let snd_check = check(snd, snd_type, context.inc_and_shift(2).insert_dec(0, fst_type));

					push_check(&mut errors, fst_check);
					push_check(&mut errors, snd_check);

					wrap_checks(errors)
				},
				_ => Err(vec![Error::new(term, context, ExpectedOfPairType { giv_type: type_ann })])
			}
		},
		PairElim(ref discrim, ref body) => {
			let mut errors = Vec::new();

			let discrim_type = discrim.r#type(context.clone())?;
			push_check(&mut errors, check(discrim, discrim_type.clone(), context.clone()));

			match *(discrim_type.clone()).0 {
				PairTypeIntro(fst_type, snd_type) => {
					let body_context = context.clone().inc_and_shift(2).insert_dec(0, fst_type.clone()).insert_dec(1, snd_type.clone());
					let body_type = body.r#type(body_context.clone())?;
					push_check(&mut errors, check(body, body_type, body_context));
					
					push_check(&mut errors, check_usage(&term, fst_type.clone(), body, 0, context.clone()));
					push_check(&mut errors, check_usage(&term, snd_type.clone(), body, 1, context.clone()));
				}
				_ => errors.push(Error::new(discrim, context, ExpectedOfPairType { giv_type: discrim_type }))
			}

			wrap_checks(errors)
		},
		VoidTypeIntro =>
			match *(type_ann.clone()).0 {
				TypeTypeIntro(_, _) => Ok(()),
				_ => Err(vec![Error::new(term, context, ExpectedOfTypeType { min_level: 0, giv_type: type_ann.clone() })])
			},
        UnitTypeIntro =>
        	match *(type_ann.clone()).0 {
				TypeTypeIntro(_, _) => Ok(()),
				_ => Err(vec![Error::new(term, context, ExpectedOfTypeType { min_level: 0, giv_type: type_ann.clone() })])
			},
        UnitIntro =>
        	match *(type_ann.clone()).0 {
				UnitTypeIntro => Ok(()),
				_ => Err(vec![Error::new(term, context, ExpectedOfUnitType { giv_type: type_ann.clone() })])
			},
		DoubTypeIntro =>
			match *(type_ann.clone()).0 {
				TypeTypeIntro(_, _) => Ok(()),
				_ => Err(vec![Error::new(term, context, ExpectedOfTypeType { min_level: 0, giv_type: type_ann.clone() })])
			},
		DoubIntro(_) =>
			match *(type_ann.clone()).0 {
				EnumTypeIntro => Ok(()),
				_ => Err(vec![Error::new(term, context, ExpectedOfEnumType { giv_type: type_ann.clone() })])
			},
		DoubElim(ref discrim, ref branch1, ref branch2) => {
			let mut errors = Vec::new();

			let discrim_type = discrim.r#type(context.clone())?;
			push_check(&mut errors, check(discrim, discrim_type.clone(), context.clone()));

			match *(discrim_type.clone()).0 {
				DoubTypeIntro => {
					let branch_context = |d: Term|
						match *normalize(discrim.clone(), context.clone()).0 { // updates context with the now known value of discrim if it is a var
							Var(index) => context.clone().update(index, d.clone()).insert_def(index, d),
							_ => context.clone()
						};

					push_check(&mut errors, check(branch1, type_ann.clone(), branch_context(discrim.clone())));
					push_check(&mut errors, check(branch2, type_ann, branch_context(discrim.clone())));
				},
				_ => errors.push(Error::new(discrim, context, ExpectedOfEnumType { giv_type: discrim_type }))
			}

			wrap_checks(errors)
		},
		FoldTypeIntro(ref inner_type) =>
			match *(type_ann.clone()).0 {
				TypeTypeIntro(_, _) => {
					let mut errors = Vec::new();
					push_check(&mut errors, check(inner_type, inner_type.r#type(context.clone())?, context.clone()));
					push_check(&mut errors, check_type(inner_type, context.clone()));
					wrap_checks(errors)
				},
				_ => Err(vec![Error::new(term, context, ExpectedOfTypeType { min_level: 0, giv_type: type_ann.clone() })])
			},
		FoldIntro(ref inner_term) =>
			match *(type_ann.clone()).0 {
				FoldTypeIntro(inner_type) => check(inner_term, inner_type, context),
				_ => Err(vec![Error::new(term, context, ExpectedOfFoldType { giv_type: type_ann.clone() })])
			},
		FoldElim(ref folded_term) => {
			let mut errors = Vec::new();
			let folded_term_type = folded_term.r#type(context.clone())?;

			push_check(
				&mut errors,
				if is_terms_eq(&folded_term_type, &type_ann) {
					Ok(())
				} else {
					Err(vec![Error::new(term, context, MismatchedTypes { exp_type: type_ann.clone(), giv_type: folded_term_type.clone() })])
				});

			wrap_checks(errors)
		},
		CapturesListTypeIntro(level) =>
			match *type_ann.clone().0 {
				TypeTypeIntro(u_level, _) =>
					if u_level > level {
						Ok(())
					} else {
						Err(vec![Error::new(term, context, ExpectedOfTypeType { min_level: level + 1, giv_type: type_ann })])
					}
				_ => Err(vec![Error::new(term, context, ExpectedOfTypeType { min_level: 1, giv_type: type_ann })])
			}
		CapturesListIntro(ref list) =>
			match *type_ann.0 {
				CapturesListTypeIntro(level) =>
					match list {
						Cons(ref head, ref tail) => {
							let mut errors = Vec::new();

							let head_type = head.r#type(context.clone())?;
							let tail_type = head.r#type(context.clone())?;

							match (*head_type.clone().0, *tail_type.clone().0) {
								(TypeTypeIntro(_, head_usage), CapturesListTypeIntro(_)) => {
									push_check(&mut errors, check(head, Term(Box::new(TypeTypeIntro(level, head_usage)), None), context.clone()));
									push_check(&mut errors, check_type(head, context.clone()));
									let caps_list_type =
										Term(Box::new(
											CapturesListTypeIntro(level)),
											Some(Box::new(Term(Box::new(
												TypeTypeIntro(level, Usage::Unique)), // TODO: figure out whether this is correct
												None))));
									push_check(&mut errors, check(tail, caps_list_type, context));
								}
								(TypeTypeIntro(level, _), _) => errors.push(Error::new(head, context, ExpectedOfCapturesListType { min_level: level, giv_type: head_type })),
								(_, CapturesListTypeIntro(level)) => errors.push(Error::new(tail, context, ExpectedOfTypeType { min_level: level, giv_type: tail_type })),
								(_, _) => {
									errors.push(Error::new(head, context.clone(), ExpectedOfTypeType { min_level: 0, giv_type: head_type }));
									errors.push(Error::new(tail, context, ExpectedOfCapturesListType { min_level: 0, giv_type: tail_type }));
								}
							}

							wrap_checks(errors)
						},
						Nil => Ok(())
					}
				_ => Err(vec![Error::new(term, context, ExpectedOfCapturesListType { min_level: 0, giv_type: type_ann })])
			}
	}
}