// Copyright (c) 2021 10X Genomics, Inc. All rights reserved.

// Test for consistency between VDJ cells and GEX cells.  This is designed to work even if
// NCELL is used.  We take up to 100 VDJ cells having both heavy and light (or TRB and TRA)
// chains, and having the highest VDJ UMI count total (but using only one cell per exact
// subclonotype), and find those that are GEX cells.
//
// If n cells were taken, and k of those are GEX cells, we require that
// binomial_sum(n, k, 0.7) >= 0.00002.  For n = 100, this is the same as requiring that
// k >= 50.  Using a binomial sum threshold allows the stringency of the requirement to be
// appropriately lower when n is small.  When we tested on 260 libraries, the lowest value
// observed for k/n was 0.65, and the vast majority of values were 0.9 or higher.
//
// This code is inefficient because for every dataset, it searches the entirety of tig_bc, but
// it doesn't matter much because not much time is spent here.

use enclone_core::defs::{EncloneControl, ExactClonotype, GexInfo, TigData};
use rayon::prelude::*;
use stats_utils::binomial_sum;
use std::time::Instant;
use vector_utils::{bin_member, bin_position, reverse_sort};

pub fn test_vdj_gex_inconsistent(
    ctl: &EncloneControl,
    tig_bc: &[Vec<TigData>],
    exact_clonotypes: &[ExactClonotype],
    vdj_cells: &[Vec<String>],
    gex_info: &GexInfo,
) -> Result<(), &'static str> {
    let tinc = Instant::now();

    let mut results = Vec::<(usize, String)>::new();
    for li in 0..ctl.origin_info.n() {
        results.push((li, String::new()));
    }
    results.par_iter_mut().for_each(|res| {
        let li = res.0;
        if !ctl.origin_info.gex_path[li].is_empty() && !ctl.gen_opt.allow_inconsistent {
            let vdj = &vdj_cells[li];
            let gex = &gex_info.gex_cell_barcodes[li];
            let (mut heavy, mut light) = (vec![false; vdj.len()], vec![false; vdj.len()]);
            let mut exid = vec![0; vdj.len()];
            let mut inex = vec![false; vdj.len()];
            for (i, ex) in exact_clonotypes.iter().enumerate() {
                for clone in &ex.clones {
                    let p = bin_position(vdj, &clone[0].barcode);
                    if p >= 0 {
                        inex[p as usize] = true;
                        exid[p as usize] = i;
                    }
                }
            }
            let mut numi = vec![0; vdj.len()];
            for tigi in tig_bc {
                if tigi[0].dataset_index == li {
                    let p = bin_position(vdj, &tigi[0].barcode);
                    if p >= 0 {
                        for tig in tigi {
                            numi[p as usize] += tig.umi_count;
                            if tig.left {
                                heavy[p as usize] = true;
                            } else {
                                light[p as usize] = true;
                            }
                        }
                    }
                }
            }
            let mut x = Vec::<(usize, bool, usize)>::new();
            for i in 0..vdj.len() {
                if heavy[i] && light[i] {
                    x.push((numi[i], bin_member(gex, &vdj[i]), i));
                }
            }
            reverse_sort(&mut x);
            let mut used = vec![false; exact_clonotypes.len()];
            let (mut total, mut good) = (0, 0);
            for xi in x {
                let m = xi.2;
                if inex[m] && used[exid[m]] {
                    continue;
                }
                total += 1;
                if xi.1 {
                    good += 1;
                }
                if inex[m] {
                    used[exid[m]] = true;
                }
                if total == 100 {
                    break;
                }
            }
            if total >= 1 {
                let bino = binomial_sum(total, good, 0.7);
                if bino < 0.00002 {
                    res.1 = format!(
                        "\nThe VDJ dataset with path\n{}\nand the GEX dataset with path\n\
                        {}\nshow insufficient sharing of barcodes.  \
                        Of the {total} VDJ cells that were tested,\n\
                        only {good} were GEX cells.\n",
                        ctl.origin_info.dataset_path[li], ctl.origin_info.gex_path[li]
                    );
                }
            }
        }
    });
    let mut fail = false;
    for r in &results {
        if !r.1.is_empty() {
            fail = true;
        }
    }
    if fail {
        for r in results {
            eprint!("{}", r.1);
        }
        return Err(
            "\nThis test is restricted to VDJ cells having both chain types, uses at most \
            one cell\nper exact subclonotype, and uses up to 100 cells having the highest \
            UMI counts.\n\
            \nThe data suggest a laboratory or informatic mixup.  If you believe \
            that this is not the case,\nyou can force enclone to run by adding \
            the argument ALLOW_INCONSISTENT to the command line.\n",
        );
    }
    ctl.perf_stats(&tinc, "testing for inconsistency");
    Ok(())
}
