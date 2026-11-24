//! Tests for the process list.

use super::*;

fn proc(pid: u32, parent: Option<u32>, name: &str, cpu: f32, memory: u64) -> Proc {
    Proc { pid, parent, name: name.into(), cpu, memory, status: "Run".into() }
}

fn options(sort: Sort) -> Options {
    Options { sort, is_reversed: false, filter: String::new(), is_tree: false }
}

fn pids(procs: &[Proc], rows: &[Row]) -> Vec<u32> {
    rows.iter().map(|r| procs[r.index].pid).collect()
}

#[test]
fn load_sorts_largest_first_and_pid_smallest_first() {
    let p = [proc(1, None, "a", 1.0, 5), proc(2, None, "b", 9.0, 1), proc(3, None, "c", 4.0, 9)];
    assert_eq!(pids(&p, &rows(&p, &options(Sort::Cpu))), [2, 3, 1]);
    assert_eq!(pids(&p, &rows(&p, &options(Sort::Memory))), [3, 1, 2]);
    assert_eq!(pids(&p, &rows(&p, &options(Sort::Pid))), [1, 2, 3]);
}

#[test]
fn reversing_flips_the_order() {
    let p = [proc(1, None, "a", 1.0, 5), proc(2, None, "b", 9.0, 1)];
    let o = Options { is_reversed: true, ..options(Sort::Cpu) };
    assert_eq!(pids(&p, &rows(&p, &o)), [1, 2]);
}

#[test]
fn filter_matches_name_or_pid_and_makes_the_list_flat() {
    let p = [
        proc(1, None, "init", 0.0, 1),
        proc(2, Some(1), "Zsh", 0.0, 1),
        proc(30, Some(2), "vim", 0.0, 1),
    ];
    let o = Options { filter: "zs".into(), is_tree: true, ..options(Sort::Pid) };
    assert_eq!(pids(&p, &rows(&p, &o)), [2]);
    let o = Options { filter: "30".into(), ..options(Sort::Pid) };
    assert_eq!(pids(&p, &rows(&p, &o)), [30]);
}

#[test]
fn tree_nests_children_under_their_parent() {
    let p = [
        proc(1, None, "init", 0.0, 1),
        proc(2, Some(1), "a", 0.0, 1),
        proc(3, Some(2), "a1", 0.0, 1),
        proc(4, Some(1), "b", 0.0, 1),
    ];
    let o = Options { is_tree: true, ..options(Sort::Pid) };
    let r = rows(&p, &o);
    assert_eq!(pids(&p, &r), [1, 2, 3, 4]);
    assert_eq!(r.iter().map(|r| r.depth).collect::<Vec<_>>(), [0, 1, 2, 1]);
}

#[test]
fn a_process_whose_parent_is_missing_or_itself_is_a_root() {
    let p = [proc(5, Some(99), "orphan", 0.0, 1), proc(6, Some(6), "loop", 0.0, 1)];
    let o = Options { is_tree: true, ..options(Sort::Pid) };
    assert_eq!(rows(&p, &o).iter().map(|r| r.depth).collect::<Vec<_>>(), [0, 0]);
}

#[test]
fn sort_cycles_through_every_order() {
    let mut s = Sort::Cpu;
    for _ in 0..4 {
        s = s.next();
    }
    assert_eq!(s, Sort::Cpu);
}
