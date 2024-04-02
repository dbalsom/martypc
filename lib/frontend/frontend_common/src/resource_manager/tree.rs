/*
    MartyPC
    https://github.com/dbalsom/martypc

    Copyright 2022-2024 Daniel Balsom

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
use anyhow::{anyhow, Error};
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};

pub type FileTreeNode = TreeNode;

#[derive(Debug)]
pub enum NodeType {
    File(PathBuf),
    Directory(BTreeMap<String, TreeNode>),
}

impl Default for NodeType {
    fn default() -> Self {
        NodeType::Directory(BTreeMap::new())
    }
}

#[derive(Debug, Default)]
pub struct TreeNode {
    name: String,
    idx: usize,
    node_type: NodeType,
}

pub struct NodeDisplay {
    pub name: String,
}

impl TreeNode {
    pub(crate) fn new_directory(idx: usize, name: String) -> Self {
        TreeNode {
            name,
            idx,
            node_type: NodeType::Directory(BTreeMap::new()),
        }
    }

    fn new_file(idx: usize, name: String, path: PathBuf) -> Self {
        TreeNode {
            name,
            idx,
            node_type: NodeType::File(path),
        }
    }

    pub fn child_names(&self) -> Option<Vec<String>> {
        match &self.node_type {
            NodeType::Directory(children) => Some(children.keys().cloned().collect()),
            NodeType::File(_) => None,
        }
    }

    pub fn children(&self) -> Vec<&TreeNode> {
        match &self.node_type {
            NodeType::Directory(children) => children.values().collect(),
            NodeType::File(_) => Vec::new(),
        }
    }

    pub fn is_directory(&self) -> bool {
        match self.node_type {
            NodeType::Directory(_) => true,
            NodeType::File(_) => false,
        }
    }

    pub fn name(&self) -> String {
        match self.node_type {
            NodeType::Directory(_) => self.name.clone(),
            NodeType::File(_) => self.name.clone(),
        }
    }

    #[inline]
    pub fn idx(&self) -> usize {
        self.idx
    }

    pub fn navigate(&self, path: &mut Vec<String>) -> Result<&TreeNode, Error> {
        if path.is_empty() {
            return Err(anyhow!("Path not found"));
        }
        match self.node_type {
            NodeType::Directory(_) => {
                let component = path.pop().unwrap();
                if path.is_empty() {
                    // We are at end of path.
                    if self.name.eq_ignore_ascii_case(&component) {
                        return Ok(self);
                    }
                    else {
                        return Err(anyhow!("Path not found"));
                    }
                }
                else {
                    //let mut found_path = false;

                    // Navigate down.
                    for child in self.children() {
                        if child.name.eq_ignore_ascii_case(&component) {
                            //found_path = true;
                            return child.navigate(path);
                        }
                        else {
                            continue;
                        }
                    }
                    return Err(anyhow!("Path not found"));
                }
            }
            _ => Err(anyhow!("Cannot navigate into a file")),
        }
    }
}

pub fn build_tree(root_str: String, items: &Vec<ResourceItem>, skip: usize) -> Result<TreeNode, Error> {
    let mut root = TreeNode::new_directory(0, root_str);
    if items.is_empty() {
        return Err(anyhow!("Items vec is empty"));
    }
    for (idx, item) in items.iter().enumerate() {
        insert_path(&mut root, idx, &item.full_path, skip);
    }
    Ok(root)
}

fn insert_path(root: &mut TreeNode, idx: usize, path: &Path, skip: usize) {
    let mut current_node = root;
    for component in path.components().skip(skip) {
        // skip the root of the path
        let component_str = component.as_os_str().to_str().unwrap().to_string();
        match &mut current_node.node_type {
            NodeType::Directory(children) => {
                current_node = children.entry(component_str.clone()).or_insert_with(|| {
                    if component_str == path.file_name().unwrap().to_str().unwrap() {
                        TreeNode::new_file(idx, component_str, path.to_path_buf())
                    }
                    else {
                        TreeNode::new_directory(idx, component_str)
                    }
                });
            }
            NodeType::File(_) => break,
        }
    }
}
