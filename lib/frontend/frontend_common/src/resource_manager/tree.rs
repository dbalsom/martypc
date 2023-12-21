/*
    MartyPC
    https://github.com/dbalsom/martypc

    Copyright 2022-2023 Daniel Balsom

    Permission is hereby granted, free of charge, to any person obtaining a
    copy of this software and associated documentation files (the “Software”),
    to deal in the Software without restriction, including without limitation
    the rights to use, copy, modify, merge, publish, distribute, sublicense,
    and/or sell copies of the Software, and to permit persons to whom the
    Software is furnished to do so, subject to the following conditions:

    The above copyright notice and this permission notice shall be included in
    all copies or substantial portions of the Software.

    THE SOFTWARE IS PROVIDED “AS IS”, WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
    IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
    FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
    AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
    LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING
    FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER
    DEALINGS IN THE SOFTWARE.

    --------------------------------------------------------------------------

    frontend_common::resource_manager::tree.rs

    Create or manipulate a tree of ResourceItems from a list of paths.

*/

use crate::resource_manager::ResourceItem;
use anyhow::Error;
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};

#[derive(Debug)]
pub enum NodeType {
    File(PathBuf),
    Directory(HashMap<String, TreeNode>),
}

#[derive(Debug)]
pub struct TreeNode {
    name: String,
    node_type: NodeType,
}

impl TreeNode {
    pub(crate) fn new_directory(name: String) -> Self {
        TreeNode {
            name,
            node_type: NodeType::Directory(HashMap::new()),
        }
    }

    fn new_file(name: String, path: PathBuf) -> Self {
        TreeNode {
            name,
            node_type: NodeType::File(path),
        }
    }
}

pub fn build_tree(root_str: String, items: Vec<ResourceItem>) -> Result<TreeNode, Error> {
    let mut root = TreeNode::new_directory(root_str);
    for item in items {
        insert_path(&mut root, &item.full_path);
    }
    Ok(root)
}

fn insert_path(root: &mut TreeNode, path: &Path) {
    let mut current_node = root;
    for component in path.components().skip(1) {
        // skip the root of the path
        let component_str = component.as_os_str().to_str().unwrap().to_string();
        match &mut current_node.node_type {
            NodeType::Directory(children) => {
                current_node = children.entry(component_str.clone()).or_insert_with(|| {
                    if component_str == path.file_name().unwrap().to_str().unwrap() {
                        TreeNode::new_file(component_str, path.to_path_buf())
                    }
                    else {
                        TreeNode::new_directory(component_str)
                    }
                });
            }
            NodeType::File(_) => break,
        }
    }
}
