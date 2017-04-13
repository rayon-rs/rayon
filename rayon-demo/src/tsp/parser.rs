use std::collections::HashMap;
use std::str::{FromStr, Lines};
use regex::Regex;

use super::graph::{Graph, Node};
use super::weight::Weight;

// Parses "TSPLIB" files from sources like
//
// http://www.math.uwaterloo.ca/tsp/world/dj38.tsp
//
// The format looks roughly like this, at least for EUC_2D cases:
//
// NAME: dj38
// COMMENT : 38 locations in Djibouti
// COMMENT : Derived from National Imagery and Mapping Agency data
// COMMENT : This file is a corrected version of dj89, where duplications
// COMMENT:  have been removed.  Thanks to Jay Muthuswamy and others for
// COMMENT:  requesting data sets without duplications.
// TYPE: TSP
// DIMENSION: 38
// EDGE_WEIGHT_TYPE: EUC_2D
// NODE_COORD_SECTION
// 1 11003.611100 42102.500000

lazy_static! {
    static ref HEADER: Regex = Regex::new(r"([A-Z_]+)\s*:(.*)").unwrap();
}

lazy_static! {
    static ref COORD: Regex = Regex::new(r"([0-9]+) ([0-9.]+) ([0-9.]+)").unwrap();
}

pub fn parse_tsp_data(text: &str) -> Result<Graph, String> {
    let mut data = Data::new(text);
    let mut num_nodes = None;

    while let Some(header) = parse_tsp_header(&mut data) {
        match header.name {
            "NAME" => {}
            "COMMENT" => {}
            "TYPE" => {
                if header.value != "TSP" {
                    return Err(
                        format!(
                            "line {}: expected \"TSP\" for TYPE, not {:?}",
                            header.line,
                            header.value
                        ),
                    );
                }
            }
            "DIMENSION" => {
                match usize::from_str(header.value) {
                    Ok(n) => num_nodes = Some(n),
                    Err(_) => {
                        return Err(
                            format!(
                                "line {}: expected an integer for DIMENSION, not {:?}",
                                header.line,
                                header.value
                            ),
                        );
                    }
                }
            }
            "EDGE_WEIGHT_TYPE" => {
                if header.value != "EUC_2D" {
                    return Err(
                        format!(
                            "line {}: expected \"EUC_2D\" for EDGE_WEIGHT_TYPE, not \
                                        {:?}",
                            header.line,
                            header.value
                        ),
                    );
                }
            }
            _ => {
                return Err(format!("line {}: unknown header type {}", header.line, header.name),);
            }
        }
    }

    let num_nodes = match num_nodes {
        Some(n) => n,
        None => return Err(format!("line {}: never found DIMENSION header", data.line_num),),
    };
    let mut graph = Graph::new(num_nodes);

    if parse_coord_header(&mut data).is_none() {
        return Err(format!("line {}: expected NODE_COORD_SECTION", data.line_num),);
    }

    let mut coords = HashMap::new();
    while let Some((node, x, y)) = parse_coord(&mut data) {
        coords.insert(node, (x, y));
    }

    for i in graph.all_nodes() {
        let coord_i = match coords.get(&i) {
            Some(v) => v,
            None => {
                return Err(
                    format!(
                        "line {}: never found coordinate for node {}",
                        data.line_num,
                        i.index()
                    ),
                )
            }
        };
        for j in graph.all_nodes().filter(|&j| j != i) {
            let coord_j = match coords.get(&j) {
                Some(v) => v,
                None => {
                    return Err(
                        format!(
                            "line {}: never found coordinate for node {}",
                            data.line_num,
                            j.index()
                        ),
                    )
                }
            };

            // "For these instances, the cost of travel between cities
            // is specified by the Eulidean distance rounded to the
            // nearest whole number (the TSPLIB EUC_2D-norm)."
            let distance = (coord_i.0 - coord_j.0).powi(2) + (coord_i.1 - coord_j.1).powi(2);
            let distance = distance.sqrt();
            let distance = distance.round();
            let weight = Weight::new(distance as usize);
            graph.set_weight(i, j, weight);
        }
    }

    while let Some(()) = parse_blank(&mut data) { }

    if data.current_line.is_some() {
        return Err(format!("line {}: expected EOF", data.line_num));
    }

    Ok(graph)
}

pub struct Data<'text> {
    current_line: Option<&'text str>,
    line_num: usize,
    next_lines: Lines<'text>,
}

impl<'text> Data<'text> {
    pub fn new(data: &'text str) -> Data<'text> {
        let mut lines = data.lines();
        let current_line = lines.next();
        Data {
            current_line: current_line,
            line_num: 1,
            next_lines: lines,
        }
    }

    pub fn advance(&mut self) {
        self.current_line = self.next_lines.next();
        self.line_num += 1;
    }
}

pub struct Header<'text> {
    line: usize,
    name: &'text str,
    value: &'text str,
}

pub fn parse_tsp_header<'text>(data: &mut Data<'text>) -> Option<Header<'text>> {
    data.current_line
        .and_then(
            |current_line| {
                HEADER
                    .captures(current_line)
                    .map(
                        |captures| {
                            data.advance();
                            Header {
                                line: data.line_num - 1,
                                name: captures.get(1).unwrap().as_str().trim(),
                                value: captures.get(2).unwrap().as_str().trim(),
                            }
                        },
                    )
            },
        )
}

pub fn parse_coord_header<'text>(data: &mut Data<'text>) -> Option<()> {
    data.current_line
        .and_then(
            |current_line| if current_line == "NODE_COORD_SECTION" {
                data.advance();
                Some(())
            } else {
                None
            },
        )
}

pub fn parse_coord<'text>(data: &mut Data<'text>) -> Option<(Node, f64, f64)> {
    // 1 11003.611100 42102.500000
    data.current_line
        .and_then(
            |current_line| {
                COORD
                    .captures(current_line)
                    .and_then(
                        |captures| {
                            usize::from_str(captures.get(1).unwrap().as_str())
                                .ok()
                                .and_then(
                                    |n| {
                                        f64::from_str(captures.get(2).unwrap().as_str())
                                            .ok()
                                            .and_then(
                                                |x| {
                                                    f64::from_str(captures.get(3).unwrap().as_str())
                                                        .ok()
                                                        .and_then(
                                                            |y| if n > 0 {
                                                                data.advance();
                                                                Some((Node::new(n - 1), x, y))
                                                            } else {
                                                                None
                                                            },
                                                        )
                                                },
                                            )
                                    },
                                )
                        },
                    )
            },
        )
}

pub fn parse_blank<'text>(data: &mut Data<'text>) -> Option<()> {
    data.current_line
        .and_then(
            |current_line| if current_line.trim().is_empty() {
                data.advance();
                Some(())
            } else {
                None
            },
        )
}
