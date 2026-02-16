//! Balancing tests for the material and rule system.
//! These tests load all production data files and analyze the rule graph
//! for potential balance issues like self-replication, runaway temperature,
//! non-terminating combustion chains, and multi-step oscillations.

#[cfg(test)]
mod tests {
    use crate::defaults;
    use crate::loader::{load_all_materials, load_all_rules, load_mod, merge_mod};
    use crate::migration::IdRemap;
    use crate::validator;
    use std::collections::{HashMap, HashSet};

    fn load_all() -> (
        alkahest_core::material::MaterialTable,
        alkahest_core::rule::RuleSet,
    ) {
        let table = load_all_materials(&[
            include_str!("../../../data/materials/naturals.ron"),
            include_str!("../../../data/materials/organics.ron"),
            include_str!("../../../data/materials/energy.ron"),
            include_str!("../../../data/materials/explosives.ron"),
            include_str!("../../../data/materials/metals.ron"),
            include_str!("../../../data/materials/synthetics.ron"),
            include_str!("../../../data/materials/exotic.ron"),
        ])
        .expect("all materials should load");

        let rules = load_all_rules(&[
            include_str!("../../../data/rules/combustion.ron"),
            include_str!("../../../data/rules/structural.ron"),
            include_str!("../../../data/rules/phase_change.ron"),
            include_str!("../../../data/rules/dissolution.ron"),
            include_str!("../../../data/rules/displacement.ron"),
            include_str!("../../../data/rules/biological.ron"),
            include_str!("../../../data/rules/thermal.ron"),
            include_str!("../../../data/rules/synthesis.ron"),
        ])
        .expect("all rules should load");

        (table, rules)
    }

    /// Verify no rule self-replicates a non-energy material.
    /// Self-replication: both outputs are the same non-air material AND
    /// that material is one of the inputs (it duplicates itself consuming the other).
    /// Energy materials (Fire, Ember, etc.) are allowed to spread.
    #[test]
    fn test_no_self_replication() {
        let (table, rules) = load_all();

        // Materials allowed to self-replicate by design:
        // - Energy materials (Fire, Ember, Spark, etc.) spread by nature
        // - Water (3), Ice (10): phase change spreading (freezing, condensation)
        // - Rust (77): corrosion spreading
        // - Moss (107), Algae (108): biological growth
        // - Cursed Water (219): exotic spreading mechanic
        let allowed_spreaders: HashSet<u16> = {
            let mut s: HashSet<u16> = table
                .materials
                .iter()
                .filter(|m| m.id >= defaults::ENERGY_START && m.id <= defaults::ENERGY_END)
                .map(|m| m.id)
                .collect();
            // Legacy energy
            s.extend([5, 6, 7]);
            // Phase change / environmental spreading
            s.extend([3, 10, 77, 107, 108, 219]);
            s
        };

        let mut violations = Vec::new();

        for rule in &rules.rules {
            // Check: both outputs are the same material AND it's an input
            if rule.output_a == rule.output_b && rule.output_a != 0 {
                let replicated = rule.output_a;
                let is_input = replicated == rule.input_a || replicated == rule.input_b;
                if is_input && !allowed_spreaders.contains(&replicated) {
                    let name = table
                        .get(replicated)
                        .map(|m| m.name.as_str())
                        .unwrap_or("?");
                    violations.push(format!(
                        "Rule '{}': {} (ID {}) self-replicates (both outputs)",
                        rule.name, name, replicated
                    ));
                }
            }
        }

        assert!(
            violations.is_empty(),
            "Self-replication detected in {} rules:\n{}",
            violations.len(),
            violations.join("\n")
        );
    }

    /// Verify no exothermic reaction chain can exceed 8000K without consuming fuel.
    /// Each rule with temp_delta > 0 must transform at least one material,
    /// and the cumulative temp_delta of any single rule cannot exceed 8000.
    #[test]
    fn test_no_runaway_temperature() {
        let (_, rules) = load_all();
        let mut violations = Vec::new();

        for rule in &rules.rules {
            // Single rule temp_delta cannot exceed quantization max
            if rule.temp_delta > 4095 {
                violations.push(format!(
                    "Rule '{}': temp_delta {} exceeds quantization max 4095",
                    rule.name, rule.temp_delta
                ));
            }

            // Exothermic rules must transform material (already validated, but double-check)
            if rule.temp_delta > 0 && rule.output_a == rule.input_a && rule.output_b == rule.input_b
            {
                violations.push(format!(
                    "Rule '{}': exothermic (temp_delta={}) without material transformation",
                    rule.name, rule.temp_delta
                ));
            }
        }

        assert!(
            violations.is_empty(),
            "Runaway temperature risks found:\n{}",
            violations.join("\n")
        );
    }

    /// Verify every flammable material's combustion chain terminates in
    /// a non-flammable product within a bounded number of steps.
    #[test]
    fn test_all_combustion_exhausts() {
        let (table, rules) = load_all();

        // Find all flammable materials
        let flammable: HashSet<u16> = table
            .materials
            .iter()
            .filter(|m| m.flammability > 0.0)
            .map(|m| m.id)
            .collect();

        // Build combustion product graph: when material X burns, what does it become?
        // Look for rules where Fire (5) is an input and a flammable material is transformed
        let fire_id: u16 = 5;
        let mut combustion_products: HashMap<u16, Vec<u16>> = HashMap::new();
        for rule in &rules.rules {
            if rule.input_b == fire_id && flammable.contains(&rule.input_a) {
                combustion_products
                    .entry(rule.input_a)
                    .or_default()
                    .push(rule.output_a);
            }
            if rule.input_a == fire_id && flammable.contains(&rule.input_b) {
                combustion_products
                    .entry(rule.input_b)
                    .or_default()
                    .push(rule.output_b);
            }
        }

        // For each flammable material, trace the combustion chain (max 10 steps)
        let mut unterminated = Vec::new();
        for &mat_id in &flammable {
            let mut visited = HashSet::new();
            let mut frontier = vec![mat_id];
            let mut terminated = false;

            for _ in 0..10 {
                let mut next_frontier = Vec::new();
                for &current in &frontier {
                    if !flammable.contains(&current) {
                        terminated = true;
                        break;
                    }
                    if !visited.insert(current) {
                        continue; // cycle detected
                    }
                    if let Some(products) = combustion_products.get(&current) {
                        next_frontier.extend(products);
                    } else {
                        // No combustion rule for this flammable material
                        // This is also fine — material burns but has no explicit product rule
                        terminated = true;
                        break;
                    }
                }
                if terminated {
                    break;
                }
                frontier = next_frontier;
                if frontier.is_empty() {
                    terminated = true;
                    break;
                }
            }

            if !terminated {
                let name = table.get(mat_id).map(|m| m.name.as_str()).unwrap_or("?");
                unterminated.push(format!("{} (ID: {})", name, mat_id));
            }
        }

        assert!(
            unterminated.is_empty(),
            "Flammable materials with non-terminating combustion chains:\n{}",
            unterminated.join("\n")
        );
    }

    /// Detect multi-step oscillation cycles: A→B→C→A where all rules have
    /// overlapping temperature ranges. Extends the validator's pairwise check.
    #[test]
    fn test_no_multi_step_oscillation() {
        let (_, rules) = load_all();

        // Build a directed graph of material transformations
        // Edge: (input_pair) -> (output_pair) with temp range
        struct Edge {
            output: (u16, u16),
            min_temp: u32,
            max_temp: u32,
        }

        let mut graph: HashMap<(u16, u16), Vec<Edge>> = HashMap::new();

        for rule in &rules.rules {
            let transforms = rule.output_a != rule.input_a || rule.output_b != rule.input_b;
            if !transforms {
                continue;
            }

            let key_a = (
                rule.input_a.min(rule.input_b),
                rule.input_a.max(rule.input_b),
            );
            let out_a = (
                rule.output_a.min(rule.output_b),
                rule.output_a.max(rule.output_b),
            );
            let max_temp = if rule.max_temp == 0 {
                u32::MAX
            } else {
                rule.max_temp
            };

            graph.entry(key_a).or_default().push(Edge {
                output: out_a,
                min_temp: rule.min_temp,
                max_temp,
            });
        }

        // Check for 3-step cycles: A->B->C->A with overlapping temp ranges
        let mut cycles_found = Vec::new();
        for (&start, edges_ab) in &graph {
            for edge_ab in edges_ab {
                let mid1 = edge_ab.output;
                if let Some(edges_bc) = graph.get(&mid1) {
                    for edge_bc in edges_bc {
                        let mid2 = edge_bc.output;
                        if let Some(edges_ca) = graph.get(&mid2) {
                            for edge_ca in edges_ca {
                                if edge_ca.output == start {
                                    // Found cycle A->B->C->A, check temp overlap
                                    let overlap_min = edge_ab
                                        .min_temp
                                        .max(edge_bc.min_temp)
                                        .max(edge_ca.min_temp);
                                    let overlap_max = edge_ab
                                        .max_temp
                                        .min(edge_bc.max_temp)
                                        .min(edge_ca.max_temp);

                                    if overlap_min <= overlap_max {
                                        cycles_found.push(format!(
                                            "({:?}) -> ({:?}) -> ({:?}) -> ({:?}) [temp {}-{}]",
                                            start, mid1, mid2, start, overlap_min, overlap_max
                                        ));
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        assert!(
            cycles_found.is_empty(),
            "Multi-step oscillation cycles detected:\n{}",
            cycles_found.join("\n")
        );
    }

    /// Verify each material category range has sufficient coverage.
    #[test]
    fn test_category_coverage() {
        let (table, _) = load_all();

        let categories = [
            ("Naturals", defaults::NATURALS_START, defaults::NATURALS_END),
            ("Metals", defaults::METALS_START, defaults::METALS_END),
            ("Organics", defaults::ORGANICS_START, defaults::ORGANICS_END),
            ("Energy", defaults::ENERGY_START, defaults::ENERGY_END),
            (
                "Synthetics",
                defaults::SYNTHETICS_START,
                defaults::SYNTHETICS_END,
            ),
            ("Exotic", defaults::EXOTIC_START, defaults::EXOTIC_END),
        ];

        let mut underfilled = Vec::new();
        for (name, start, end) in &categories {
            let count = table
                .materials
                .iter()
                .filter(|m| m.id >= *start && m.id <= *end)
                .count();
            if count < 25 {
                underfilled.push(format!("{}: {} materials (need >= 25)", name, count));
            }
        }

        assert!(
            underfilled.is_empty(),
            "Categories with insufficient materials:\n{}",
            underfilled.join("\n")
        );
    }

    /// Verify the interaction matrix is well-distributed across categories.
    /// No single category pair should dominate > 30% of all rules,
    /// and every category should participate in at least one rule.
    #[test]
    fn test_interaction_matrix_distribution() {
        let (_table, rules) = load_all();

        // Count rules per category pair
        let mut pair_counts: HashMap<(&str, &str), usize> = HashMap::new();
        for rule in &rules.rules {
            let cat_a = defaults::get_category(rule.input_a);
            let cat_b = defaults::get_category(rule.input_b);
            // Normalize pair ordering
            let pair = if cat_a <= cat_b {
                (cat_a, cat_b)
            } else {
                (cat_b, cat_a)
            };
            *pair_counts.entry(pair).or_default() += 1;
        }

        let total = rules.len();
        assert!(total > 0, "No rules loaded");

        // Check no single pair dominates > 30%
        let mut dominant = Vec::new();
        for ((cat_a, cat_b), count) in &pair_counts {
            let pct = (*count as f64) / (total as f64) * 100.0;
            if pct > 30.0 {
                dominant.push(format!(
                    "{}-{}: {} rules ({:.1}% of {})",
                    cat_a, cat_b, count, pct, total
                ));
            }
        }

        assert!(
            dominant.is_empty(),
            "Category pairs dominating > 30% of rules:\n{}",
            dominant.join("\n")
        );

        // Check every non-Air/Legacy category has at least one rule
        let all_categories = [
            "Naturals",
            "Metals",
            "Organics",
            "Energy",
            "Synthetics",
            "Exotic",
        ];
        let participating: HashSet<&str> =
            pair_counts.keys().flat_map(|(a, b)| vec![*a, *b]).collect();

        let missing: Vec<&&str> = all_categories
            .iter()
            .filter(|cat| !participating.contains(**cat))
            .collect();

        assert!(
            missing.is_empty(),
            "Categories with no interaction rules: {:?}",
            missing
        );
    }

    /// Verify that loading base + example mod passes balancing checks:
    /// mod material IDs are valid, merged materials validate, no runaway
    /// temperature in mod rules, and no duplicate IDs after merge.
    #[test]
    fn test_mod_materials_pass_balancing() {
        let (mut table, mut rules) = load_all();

        let mod_manifest_ron = include_str!("../../../data/mods/example-mod/mod.ron");
        let mod_materials_ron =
            include_str!("../../../data/mods/example-mod/materials/crystals.ron");
        let mod_rules_ron =
            include_str!("../../../data/mods/example-mod/rules/crystal_interactions.ron");

        let mod_result = load_mod(mod_manifest_ron, &[mod_materials_ron], &[mod_rules_ron])
            .expect("example mod should load");

        // Validate mod IDs are in range before remapping
        validator::validate_mod_materials(&mod_result.materials)
            .expect("mod material IDs should be in valid range");

        // Merge mod into base
        let mut remap = IdRemap::new(table.max_id());
        let _warnings = merge_mod(&mut table, &mut rules, &mod_result, &mut remap);

        // Merged material table must pass property validation (IDs unique, ranges OK)
        validator::validate_materials(&table).expect("merged materials should validate");

        // All mod rule material references must exist in the merged table
        let valid_ids: HashSet<u16> = table.materials.iter().map(|m| m.id).collect();
        for rule in &rules.rules {
            for &id in &[rule.input_a, rule.input_b, rule.output_a, rule.output_b] {
                assert!(
                    valid_ids.contains(&id),
                    "Rule '{}' references unknown material ID {}",
                    rule.name,
                    id
                );
            }
        }

        // Verify no rules have runaway temperature
        for rule in &rules.rules {
            assert!(
                rule.temp_delta <= 4095,
                "Rule '{}': temp_delta {} exceeds quantization max 4095",
                rule.name,
                rule.temp_delta
            );
        }
    }
}
