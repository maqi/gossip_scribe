use std::fs::File;
use std::io::{self, Write};
use std::io::prelude::*;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone)]
struct GossipEvent {
    index: u32,
    generation: u32,
    name: String,
    creator: String,
    self_parent: String,
    self_child: String,
    other_parent: String,
    other_children: BTreeSet<String>,
    round: BTreeMap<String, u32>,
    step: BTreeMap<String, u32>,
    estimation: BTreeMap<String, BTreeSet<bool>>,
    binary_value: BTreeMap<String, BTreeSet<bool>>,
    aux_vote: BTreeMap<String, bool>,
    decision: BTreeMap<String, bool>,
    marked: bool,
}

impl GossipEvent {
    fn equals_to(&self, other: &GossipEvent) -> bool {
        self.estimation == other.estimation && self.binary_value == other.binary_value &&
            self.decision == other.decision && self.round == other.round &&
            self.step == other.step
    }
}

impl Default for GossipEvent {
    fn default() -> GossipEvent {
        GossipEvent {
            index: 0,
            generation: 0,
            name: "".to_string(),
            creator: "".to_string(),
            self_parent: "".to_string(),
            self_child: "".to_string(),
            other_parent: "".to_string(),
            other_children: BTreeSet::new(),
            round: BTreeMap::new(),
            step: BTreeMap::new(),
            estimation: BTreeMap::new(),
            binary_value: BTreeMap::new(),
            aux_vote: BTreeMap::new(),
            decision: BTreeMap::new(),
            marked: false,
        }
    }
}

fn read(
    file: std::io::Result<File>,
) -> std::io::Result<(BTreeMap<String, GossipEvent>, Vec<String>)> {
    let mut contents = String::new();
    file?.read_to_string(&mut contents)?;

    let nodes = contents
        .split("subgraph")
        .map(|s| s.trim())
        .filter(|s| s.starts_with("cluster_"))
        .map(|s| {
            let content = s.split('{').nth(1).unwrap().split('}').nth(0).unwrap();
            let name = s.split("cluster_")
                .last()
                .unwrap()
                .split(' ')
                .nth(0)
                .unwrap();
            (name, content)
        })
        .map(|(name, content)| {
            let lines = content.split("->").map(|s| s.trim()).collect::<Vec<_>>();
            let events = lines
                .iter()
                .map(|s| if let Some(event) = s.split("\n").nth(1) {
                    event
                } else {
                    s.split(' ').nth(0).unwrap()
                })
                .collect::<Vec<_>>();
            (name, events)
        })
        .collect::<Vec<_>>();

    let lines = contents
        .split("\n\n")
        .last()
        .unwrap()
        .split("\n")
        .collect::<Vec<_>>();
    let edges = lines
        .iter()
        .filter(|s| s.len() > 6)
        .map(|s| {
            let first = s.split(" ").nth(0).unwrap();
            let second = s.split(" ").nth(2).unwrap();
            (first, second)
        })
        .collect::<Vec<_>>();

    let mut gossip_graph = BTreeMap::new();
    let mut initial_events = Vec::new();
    for &(ref node, ref events) in nodes.iter() {
        initial_events.push(events[1].to_string());
        let mut gossip_event = GossipEvent::default();
        gossip_event.name = events[1].to_string();
        gossip_event.creator = node.to_string();
        let _ = gossip_graph.insert(events[1].to_string(), gossip_event);
        for i in 2..events.len() {
            let mut gossip_event = GossipEvent::default();
            gossip_event.index = i as u32 - 1;
            gossip_event.generation = gossip_event.index;
            gossip_event.name = events[i].to_string();
            gossip_event.creator = node.to_string();
            gossip_event.self_parent = events[i - 1].to_string();

            let _ = gossip_graph.insert(events[i].to_string(), gossip_event);
            if let Some(mut event) = gossip_graph.get_mut(events[i - 1]) {
                event.self_child = events[i].to_string()
            }
        }
    }

    for edge in &edges {
        let mut index = 0;
        if let Some(mut event) = gossip_graph.get_mut(edge.0) {
            index = event.index;
            let _ = event.other_children.insert(edge.1.to_string());
        }
        if let Some(mut event) = gossip_graph.get_mut(edge.1) {
            if event.index <= index {
                index += 1;
                event.index = index;
            } else {
                index = 0;
            }
            event.other_parent = edge.0.to_string();
        }
        if index != 0 {
            // Update the other_child's all decesdants' indexes
            let other_child = gossip_graph.get(edge.1).unwrap().clone();
            update_index(&mut gossip_graph, other_child, index);
        }
    }

    crawl(&mut gossip_graph, &initial_events);

    Ok((gossip_graph, initial_events))
}

fn update_index(
    gossip_graph: &mut BTreeMap<String, GossipEvent>,
    current: GossipEvent,
    index: u32,
) {
    for other_child in &current.other_children {
        let updated_other = if let Some(child) = gossip_graph.get_mut(other_child) {
            if child.index <= index {
                child.index = index + 1;
                Some(child.clone())
            } else {
                None
            }
        } else {
            None
        };
        if let Some(child) = updated_other {
            update_index(gossip_graph, child, index + 1);
        }
    }

    let updated_self = if let Some(self_child) = gossip_graph.get_mut(&current.self_child) {
        if self_child.index <= index {
            self_child.index = index + 1;
            Some(self_child.clone())
        } else {
            None
        }
    } else {
        None
    };
    if let Some(child) = updated_self {
        update_index(gossip_graph, child, index + 1);
    }
}

fn crawl(gossip_graph: &mut BTreeMap<String, GossipEvent>, initial_events: &Vec<String>) {
    let super_majority = ((2 * initial_events.len()) / 3) + 1;
    let one_third = (initial_events.len() / 3) + 1;

    // Build True Estimations
    let mut creators = Vec::new();
    for initial in initial_events.iter() {
        let initial_event = gossip_graph.get(initial).unwrap().clone();
        let mut seen_graph: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
        build_seen_graph(&gossip_graph, initial_event.clone(), &mut seen_graph);
        creators.push(initial_event.creator.clone());

        for event in gossip_graph.iter_mut() {
            if let Some(seen_list) = seen_graph.get(event.0) {
                if seen_list.len() >= super_majority {
                    let mut estimations = BTreeSet::new();
                    let _ = estimations.insert(true);
                    let _ = event.1.estimation.insert(
                        initial_event.creator.clone(),
                        estimations,
                    );
                }
            }
        }
    }
    // Build False Estimation
    let mut false_estimated = BTreeSet::new();
    for event in gossip_graph.iter_mut() {
        let mut false_creators = BTreeSet::new();
        if event.1.estimation.len() >= super_majority &&
            event.1.estimation.len() != initial_events.len()
        {
            for creator in creators.iter() {
                if !event.1.estimation.contains_key(creator) {
                    let mut estimations = BTreeSet::new();
                    let _ = estimations.insert(false);
                    let _ = event.1.estimation.insert(creator.clone(), estimations);
                    let _ = false_creators.insert(creator.clone());
                }
            }
            let _ = false_estimated.insert((event.0.clone(), false_creators));
        }
    }
    // False estimation shall be broadcasted to others
    let mut broadcasted = BTreeMap::new();
    for entry in false_estimated {
        let mut seen_graph: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
        let event = gossip_graph.get(&entry.0).unwrap().clone();
        build_seen_graph(&gossip_graph, event, &mut seen_graph);
        for (event_name, seen_list) in &seen_graph {
            if seen_list.len() >= one_third {
                let _ = broadcasted.insert(event_name.clone(), entry.1.clone());
            }
        }
    }
    for event in gossip_graph.iter_mut() {
        if let Some(false_creators) = broadcasted.get(event.0) {
            for creator in false_creators {
                let mut estimations = if let Some(est_list) = event.1.estimation.get(creator) {
                    est_list.clone()
                } else {
                    BTreeSet::new()
                };
                let _ = estimations.insert(false);
                let _ = event.1.estimation.insert(creator.clone(), estimations);
            }
        }
    }


    let mut cur_tips = initial_events.clone();

    loop {
        for i in 0..cur_tips.len() {
            if mark(
                gossip_graph,
                &initial_events,
                cur_tips[i].clone(),
                &creators,
            )
            {
                let cur_event = gossip_graph.get(&cur_tips[i]).unwrap().clone();
                cur_tips[i] = cur_event.self_child;
            }
        }

        if cur_tips.iter().any(|tip| tip != "") {
            continue;
        } else {
            break;
        }
    }
}

fn mark(
    gossip_graph: &mut BTreeMap<String, GossipEvent>,
    initial_events: &Vec<String>,
    target: String,
    creators: &Vec<String>,
) -> bool {
    if target == "" {
        return false;
    }
    let target_event = gossip_graph.get(&target).unwrap().clone();

    let self_parent_marked =
        if let Some(self_parent) = gossip_graph.get(&target_event.self_parent) {
            self_parent.marked
        } else {
            true
        };

    let other_parent_marked =
        if let Some(other_parent) = gossip_graph.get(&target_event.other_parent) {
            other_parent.marked
        } else {
            true
        };

    if !(self_parent_marked && other_parent_marked) {
        return false;
    }

    deduce(gossip_graph, initial_events, target_event, creators);

    true
}


fn deduce(
    gossip_graph: &mut BTreeMap<String, GossipEvent>,
    initial_events: &Vec<String>,
    target: GossipEvent,
    creators: &Vec<String>,
) {
    if target.estimation.len() != initial_events.len() {
        let target_event = gossip_graph.get_mut(&target.name).unwrap();
        target_event.marked = true;
        return;
    }

    let super_majority = ((2 * initial_events.len()) / 3) + 1;
    let one_third = (initial_events.len() / 3) + 1;

    // Carry out PARSEC for each node
    for node in creators.iter() {
        let self_parent = gossip_graph.get(&target.self_parent).unwrap().clone();

        let self_parent_step = if let Some(step) = self_parent.step.get(node) {
            *step
        } else {
            0
        };
        let mut own_step = self_parent_step;
        let mut own_round = if let Some(round) = self_parent.round.get(node) {
            *round
        } else {
            0
        };

        let mut own_estimation = if let Some(estimation) = self_parent.estimation.get(node) {
            estimation.clone()
        } else {
            BTreeSet::new()
        };

        let mut estimation_seen_list: BTreeMap<bool, BTreeSet<String>> = BTreeMap::new();
        calculate_estimation_seen_list(
            gossip_graph,
            target.clone(),
            &mut estimation_seen_list,
            own_round,
            own_step,
            node.clone(),
        );
        for estimation in estimation_seen_list.iter() {
            if estimation.1.len() >= one_third {
                let _ = own_estimation.insert(*estimation.0);
            }
        }
        if own_estimation.len() == 0 {
            if let Some(estimation) = target.estimation.get(node) {
                own_estimation = estimation.clone();
            }
        }

        let mut own_decision = if let Some(decision) = self_parent.decision.get(node) {
            Some(*decision)
        } else {
            None
        };
        let mut own_aux_vote = own_decision;
        let mut own_bin_values: BTreeSet<bool> = BTreeSet::new();

        if let Some(decision) = own_decision {
            own_estimation.clear();
            let _ = own_estimation.insert(decision);
            let _ = own_bin_values.insert(decision);
        } else {
            let mut binary_value_seen_list: BTreeMap<bool, BTreeSet<String>> = BTreeMap::new();
            let mut voters = BTreeSet::new();
            let _ = voters.insert(target.creator.clone());
            if let Some(ests) = target.estimation.get(node) {
                for est in ests {
                    let _ = binary_value_seen_list.insert(*est, voters.clone());
                }
            }
            calculate_strongly_seen_bin_values(
                gossip_graph,
                target.clone(),
                &mut binary_value_seen_list,
                own_round,
                own_step,
                node.clone(),
            );

            for (est, voters) in binary_value_seen_list.iter() {
                if voters.len() < super_majority {
                    continue;
                }
                let _ = own_bin_values.insert(*est);
            }

            own_aux_vote = if let Some(aux_vote) = self_parent.aux_vote.get(node) {
                Some(aux_vote.clone())
            } else {
                if own_bin_values.len() == 0 {
                    None
                } else if own_bin_values.len() == 1 {
                    Some(*own_bin_values.iter().next().unwrap())
                } else {
                    Some(true)
                }
            };

            let mut aux_votes_seen_list: BTreeMap<bool, BTreeSet<String>> = BTreeMap::new();
            if let Some(aux_vote) = own_aux_vote {
                let mut voters = BTreeSet::new();
                let _ = voters.insert(target.creator.clone());
                let _ = aux_votes_seen_list.insert(aux_vote, voters);
            }

            calculate_strongly_seen_aux_votes(
                gossip_graph,
                target.clone(),
                &mut aux_votes_seen_list,
                own_round,
                own_step,
                node.clone(),
            );

            let mut aux_voters = BTreeSet::new();
            for voters in aux_votes_seen_list.values() {
                for voter in voters {
                    let _ = aux_voters.insert(voter.clone());
                }
            }

            if own_step == 0 {
                if aux_voters.len() >= super_majority {
                    for (aux_vote, voters) in aux_votes_seen_list.iter() {
                        let est = if voters.len() >= super_majority {
                            if *aux_vote {
                                own_decision = Some(true);
                                continue;
                            } else {
                                false
                            }
                        } else {
                            true
                        };

                        own_estimation.clear();
                        let _ = own_estimation.insert(est);
                    }
                    own_step += 1;
                }
            } else if own_step == 1 {
                if aux_voters.len() >= super_majority {
                    for (aux_vote, voters) in aux_votes_seen_list.iter() {
                        let est = if voters.len() >= super_majority {
                            if !*aux_vote {
                                own_decision = Some(false);
                                continue;
                            } else {
                                true
                            }
                        } else {
                            false
                        };

                        own_estimation.clear();
                        let _ = own_estimation.insert(est);
                    }
                    own_step += 1;
                }
            } else {
                if aux_voters.len() >= super_majority {
                    for (aux_vote, voters) in aux_votes_seen_list.iter() {
                        let est = if voters.len() >= super_majority {
                            *aux_vote
                        } else {
                            //TODO: deploy genuinely flipped concrete coin
                            true
                        };

                        own_estimation.clear();
                        let _ = own_estimation.insert(est);
                    }
                    own_step = 0;
                    own_round += 1;
                }
            }
        }
        if own_step != self_parent_step {
            own_bin_values.clear();
            own_aux_vote = None;
        }

        if let Some(event) = gossip_graph.get_mut(&target.name) {
            let _ = event.estimation.insert(node.clone(), own_estimation);
            let _ = event.binary_value.insert(node.clone(), own_bin_values);
            if let Some(aux_vote) = own_aux_vote {
                let _ = event.aux_vote.insert(node.clone(), aux_vote);
            }
            if let Some(decision) = own_decision {
                let _ = event.decision.insert(node.clone(), decision);
            }
            event.marked = true;
            let _ = event.step.insert(node.clone(), own_step);
            let _ = event.round.insert(node.clone(), own_round);
        }
    }
}

fn calculate_estimation_seen_list(
    gossip_graph: &BTreeMap<String, GossipEvent>,
    tip: GossipEvent,
    estimation_seen_list: &mut BTreeMap<bool, BTreeSet<String>>,
    round: u32,
    step: u32,
    whom: String,
) {
    if let Some(self_parent) = gossip_graph.get(&tip.self_parent) {
        let self_parent_round = if let Some(round) = self_parent.round.get(&whom) {
            *round
        } else {
            0
        };
        let self_parent_step = if let Some(step) = self_parent.step.get(&whom) {
            *step
        } else {
            0
        };
        if self_parent_round >= round && self_parent_step >= step {
            if let Some(estimations) = self_parent.estimation.get(&whom) {
                for estimation in estimations {
                    let mut voters = if let Some(voters) = estimation_seen_list.get(&estimation) {
                        voters.clone()
                    } else {
                        BTreeSet::new()
                    };
                    let _ = voters.insert(self_parent.creator.clone());
                    let _ = estimation_seen_list.insert(*estimation, voters.clone());
                }
            }
            calculate_estimation_seen_list(
                gossip_graph,
                self_parent.clone(),
                estimation_seen_list,
                round,
                step,
                whom.clone(),
            );
        }
    }
    if let Some(other_parent) = gossip_graph.get(&tip.other_parent) {
        let other_parent_round = if let Some(round) = other_parent.round.get(&whom) {
            *round
        } else {
            0
        };
        let other_parent_step = if let Some(step) = other_parent.step.get(&whom) {
            *step
        } else {
            0
        };
        if other_parent_round >= round && other_parent_step >= step {
            if let Some(estimations) = other_parent.estimation.get(&whom) {
                for estimation in estimations {
                    let mut voters = if let Some(voters) = estimation_seen_list.get(&estimation) {
                        voters.clone()
                    } else {
                        BTreeSet::new()
                    };
                    let _ = voters.insert(other_parent.creator.clone());
                    let _ = estimation_seen_list.insert(*estimation, voters.clone());
                }
            }
            calculate_estimation_seen_list(
                gossip_graph,
                other_parent.clone(),
                estimation_seen_list,
                round,
                step,
                whom.clone(),
            );
        }
    }
}

fn calculate_strongly_seen_aux_votes(
    gossip_graph: &BTreeMap<String, GossipEvent>,
    tip: GossipEvent,
    aux_votes_seen_list: &mut BTreeMap<bool, BTreeSet<String>>,
    round: u32,
    step: u32,
    whom: String,
) {
    if let Some(self_parent) = gossip_graph.get(&tip.self_parent) {
        let self_parent_round = if let Some(round) = self_parent.round.get(&whom) {
            *round
        } else {
            0
        };
        let self_parent_step = if let Some(step) = self_parent.step.get(&whom) {
            *step
        } else {
            0
        };
        if self_parent_round >= round && self_parent_step >= step {
            if let Some(aux_vote) = self_parent.aux_vote.get(&whom) {
                let mut voters = if let Some(voters) = aux_votes_seen_list.get(&aux_vote) {
                    voters.clone()
                } else {
                    BTreeSet::new()
                };
                let _ = voters.insert(self_parent.creator.clone());
                let _ = aux_votes_seen_list.insert(*aux_vote, voters.clone());
            }
            calculate_strongly_seen_aux_votes(
                gossip_graph,
                self_parent.clone(),
                aux_votes_seen_list,
                round,
                step,
                whom.clone(),
            );
        }
    }
    if let Some(other_parent) = gossip_graph.get(&tip.other_parent) {
        let other_parent_round = if let Some(round) = other_parent.round.get(&whom) {
            *round
        } else {
            0
        };
        let other_parent_step = if let Some(step) = other_parent.step.get(&whom) {
            *step
        } else {
            0
        };
        if other_parent_round >= round && other_parent_step >= step {
            if let Some(aux_vote) = other_parent.aux_vote.get(&whom) {
                let mut voters = if let Some(voters) = aux_votes_seen_list.get(&aux_vote) {
                    voters.clone()
                } else {
                    BTreeSet::new()
                };
                let _ = voters.insert(other_parent.creator.clone());
                let _ = aux_votes_seen_list.insert(*aux_vote, voters.clone());
            }
            calculate_strongly_seen_aux_votes(
                gossip_graph,
                other_parent.clone(),
                aux_votes_seen_list,
                round,
                step,
                whom.clone(),
            );
        }
    }
}

fn calculate_strongly_seen_bin_values(
    gossip_graph: &BTreeMap<String, GossipEvent>,
    tip: GossipEvent,
    binary_value_seen_list: &mut BTreeMap<bool, BTreeSet<String>>,
    round: u32,
    step: u32,
    whom: String,
) {
    if let Some(self_parent) = gossip_graph.get(&tip.self_parent) {
        let self_parent_round = if let Some(round) = self_parent.round.get(&whom) {
            *round
        } else {
            0
        };
        let self_parent_step = if let Some(step) = self_parent.step.get(&whom) {
            *step
        } else {
            0
        };
        if self_parent_round >= round && self_parent_step >= step {
            if let Some(ests) = self_parent.estimation.get(&whom) {
                for est in ests {
                    let mut voters = if let Some(voters) = binary_value_seen_list.get(est) {
                        voters.clone()
                    } else {
                        BTreeSet::new()
                    };
                    let _ = voters.insert(self_parent.creator.clone());
                    let _ = binary_value_seen_list.insert(*est, voters);
                }
            }
            calculate_strongly_seen_bin_values(
                gossip_graph,
                self_parent.clone(),
                binary_value_seen_list,
                round,
                step,
                whom.clone(),
            );
        }
    }
    if let Some(other_parent) = gossip_graph.get(&tip.other_parent) {
        let other_parent_round = if let Some(round) = other_parent.round.get(&whom) {
            *round
        } else {
            0
        };
        let other_parent_step = if let Some(step) = other_parent.step.get(&whom) {
            *step
        } else {
            0
        };
        if other_parent_round >= round && other_parent_step >= step {
            if let Some(ests) = other_parent.estimation.get(&whom) {
                for est in ests {
                    let mut voters = if let Some(voters) = binary_value_seen_list.get(est) {
                        voters.clone()
                    } else {
                        BTreeSet::new()
                    };
                    let _ = voters.insert(other_parent.creator.clone());
                    let _ = binary_value_seen_list.insert(*est, voters);
                }
            }
            calculate_strongly_seen_bin_values(
                gossip_graph,
                other_parent.clone(),
                binary_value_seen_list,
                round,
                step,
                whom.clone(),
            );
        }
    }
}


// For each event, build the graph that on each gossip_event being decendants of it,
// the list of nodes that sees that event
fn build_seen_graph(
    gossip_graph: &BTreeMap<String, GossipEvent>,
    current: GossipEvent,
    seen_graph: &mut BTreeMap<String, BTreeSet<String>>,
) {
    let cur_seen_list = if let Some(seen_list) = seen_graph.get(&current.name) {
        seen_list.clone()
    } else {
        BTreeSet::new()
    };

    for child in &current.other_children {
        if let Some(other_child) = gossip_graph.get(child) {
            let mut other_seen_list = if let Some(seen_list) = seen_graph.get(&other_child.name) {
                seen_list.clone()
            } else {
                BTreeSet::new()
            };
            let _ = other_seen_list.insert(other_child.creator.clone());
            let combined = cur_seen_list.union(&other_seen_list).cloned().collect();
            let _ = seen_graph.insert(other_child.name.clone(), combined);
            build_seen_graph(gossip_graph, other_child.clone(), seen_graph);
        }
    }

    if let Some(self_child) = gossip_graph.get(&current.self_child) {
        let mut self_seen_list = if let Some(seen_list) = seen_graph.get(&self_child.name) {
            seen_list.clone()
        } else {
            BTreeSet::new()
        };
        let _ = self_seen_list.insert(self_child.creator.clone());
        let combined = cur_seen_list.union(&self_seen_list).cloned().collect();
        let _ = seen_graph.insert(self_child.name.clone(), combined);
        build_seen_graph(gossip_graph, self_child.clone(), seen_graph);
    }
}

fn write_self_parents<T: Write>(
    out: &mut T,
    node: String,
    gossip_graph: &BTreeMap<String, GossipEvent>,
    events: &Vec<GossipEvent>,
) -> io::Result<()> {
    writeln!(out, "    {:?} [style=invis]", node)?;
    for event in events {
        if event.self_parent == "" {
            writeln!(out, "    {:?} -> \"{}\" [style=invis]", node, event.name)?
        } else {
            let self_parent = gossip_graph.get(&event.self_parent).unwrap();
            if event.index <= (self_parent.index + 1) {
                writeln!(out, "    \"{}\" -> \"{}\"", event.self_parent, event.name)?
            } else {
                let gap = event.index - self_parent.index;
                writeln!(
                    out,
                    "    \"{}\" -> \"{}\" [minlen={}]",
                    event.self_parent,
                    event.name,
                    gap
                )?
            }
        }
    }
    Ok(())
}

fn write_subgraph<T: Write>(
    out: &mut T,
    node: String,
    gossip_graph: &BTreeMap<String, GossipEvent>,
    events: &Vec<GossipEvent>,
) -> io::Result<()> {
    writeln!(out, "  subgraph cluster_{} {{", node)?;
    writeln!(out, "    label={:?}", node)?;

    write_self_parents(out, node, &gossip_graph, events)?;
    writeln!(out, "")?;
    writeln!(out, "  }}")
}

fn write_other_parents<T: Write>(out: &mut T, events: &Vec<GossipEvent>) -> io::Result<()> {
    // Write the communications between events
    for event in events {
        if event.other_parent != "" {
            writeln!(
                out,
                "  \"{}\" -> \"{}\" [constraint=false]",
                event.other_parent,
                event.name
            )?;
        }
    }
    Ok(())
}

fn write_nodes<T: Write>(out: &mut T, nodes: Vec<String>) -> io::Result<()> {
    writeln!(out, "  {{")?;
    writeln!(out, "    rank=same")?;
    for node in &nodes {
        writeln!(out, "    {:?} [style=filled, color=white]", node)?;

    }
    writeln!(out, "  }}")?;
    // Order the nodes alphabetically
    write!(out, "  ")?;
    let mut index = 0;
    for node in &nodes {
        write!(out, "{:?}", node)?;
        if index < nodes.len() - 1 {
            write!(out, " -> ")?;
            index += 1;
        }
    }
    writeln!(out, " [style=invis]")
}

fn write_evaluates<T: Write>(
    out: &mut T,
    initial_events: Vec<String>,
    gossip_graph: BTreeMap<String, GossipEvent>,
) -> io::Result<()> {
    for event in gossip_graph.values() {
        // write!(out, " {} ", event.name)?;
        // writeln!(out, " [label=\"Index: {}\"]", event.index)?;

        if event.estimation.len() == initial_events.len() {
            {
                let self_parent = gossip_graph.get(&event.self_parent).unwrap();
                if event.equals_to(self_parent) {
                    writeln!(
                        out,
                        " {} [label=\"{}_{}\"]",
                        event.name,
                        event.creator.chars().next().unwrap(),
                        event.generation
                    )?;
                    continue;
                }
            }

            writeln!(out, " {} [shape=rectangle]", event.name)?;

            write!(out, " {} ", event.name)?;
            write!(
                out,
                " [label=\"{}_{}",
                event.creator.chars().next().unwrap(),
                event.generation
            )?;

            write!(out, "\nRound: [")?;
            for round in &event.round {
                write!(out, " {}:{} ", round.0.chars().next().unwrap(), round.1)?;
            }
            write!(out, "]")?;

            write!(out, "\nStep: [")?;
            for step in &event.step {
                write!(out, " {}:{} ", step.0.chars().next().unwrap(), step.1)?;
            }
            write!(out, "]")?;

            write!(out, "\nEst: [")?;
            for est in &event.estimation {
                write!(out, "{}:{{", est.0.chars().next().unwrap())?;
                for estimate in est.1 {
                    if *estimate {
                        write!(out, "t")?;
                    } else {
                        if est.1.len() > 1 {
                            write!(out, "f,")?;
                        } else {
                            write!(out, "f")?;
                        }
                    }
                }
                write!(out, "}} ")?;
            }
            write!(out, "]")?;
            if event.binary_value.len() > 0 {
                write!(out, "\nBin: [")?;
                for bin_value in &event.binary_value {
                    write!(out, "{}:{{", bin_value.0.chars().next().unwrap())?;
                    for bool_value in bin_value.1 {
                        if *bool_value {
                            write!(out, "t")?;
                        } else {
                            if bin_value.1.len() > 1 {
                                write!(out, "f,")?;
                            } else {
                                write!(out, "f")?;
                            }
                        }
                    }
                    write!(out, "}} ")?;
                }
                write!(out, "]")?;
                write!(out, "\nAux: [")?;
                for aux_vote in &event.aux_vote {
                    if *aux_vote.1 {
                        write!(out, "{}:{{t}} ", aux_vote.0.chars().next().unwrap())?;
                    } else {
                        write!(out, "{}:{{f}} ", aux_vote.0.chars().next().unwrap())?;
                    }
                }
                write!(out, "]")?;
                if event.decision.len() > 0 {
                    write!(out, "\nDec: [")?;
                    for decision in &event.decision {
                        if *decision.1 {
                            write!(out, "{}:{{t}} ", decision.0.chars().next().unwrap())?;
                        } else {
                            write!(out, "{}:{{f}} ", decision.0.chars().next().unwrap())?;
                        }
                    }
                    write!(out, "]")?;
                }
            }
            writeln!(out, "\"]")?;
        } else {
            writeln!(
                out,
                " {} [label=\"{}_{}\"]",
                event.name,
                event.creator.chars().next().unwrap(),
                event.generation
            )?;
        }
    }

    let super_majority = ((2 * initial_events.len()) / 3) + 1;
    // For each creator, there shall be only one observor
    for initial in initial_events.iter() {
        let mut target_event = gossip_graph.get(initial).unwrap().clone();
        loop {
            target_event = if let Some(child) = gossip_graph.get(&target_event.self_child) {
                if child.binary_value.len() >= super_majority {
                    writeln!(out, " {} [style=filled, fillcolor=beige]", child.name)?;
                    break;
                }
                child.clone()
            } else {
                break;
            };
        }
    }

    writeln!(out, "")
}

fn write_gossip_graph_dot<T: Write>(
    out: &mut T,
    gossip_graph: BTreeMap<String, GossipEvent>,
    initial_events: Vec<String>,
) -> io::Result<()> {
    let mut nodes = Vec::new();
    for initial in initial_events.iter() {
        let initial_event = gossip_graph.get(initial).unwrap().clone();
        nodes.push(initial_event.creator.clone());
    }

    writeln!(out, "digraph GossipGraph {{")?;
    writeln!(out, "  splines=false")?;
    writeln!(out, "  rankdir=BT")?;

    for node in &nodes {
        let mut events: Vec<_> = gossip_graph
            .values()
            .filter(|event| event.creator == *node)
            .cloned()
            .collect();
        events.sort_by_key(|event| event.index);
        write_subgraph(out, node.clone(), &gossip_graph, &events)?;
        write_other_parents(out, &events)?;
    }

    write_evaluates(out, initial_events, gossip_graph)?;

    write_nodes(out, nodes)?;
    writeln!(out, "}}")
}


/// Output a graphviz of the gossip graph to a file named `gossip_graph.dot`.
fn write(
    gossip_graph: BTreeMap<String, GossipEvent>,
    initial_events: Vec<String>,
) -> io::Result<()> {
    let mut file = File::create("gossip_graph.dot")?;
    write_gossip_graph_dot(&mut file, gossip_graph, initial_events)
}


fn main() {
    // TODO: take input file from CLI args with clap
    // Also take type of annotation to produce from there
    let input_filename = "input.dot";
    let (gossip_graph, initial_events) = read(File::open(input_filename)).unwrap();
    write(gossip_graph, initial_events).unwrap();
}
