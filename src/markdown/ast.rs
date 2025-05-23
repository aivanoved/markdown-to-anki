use std::{rc::Rc, usize};

#[derive(Debug, Clone)]
pub enum Text {
    Plain(String),
    Italic(String),
    Bold(String),
    Strikethrough(String),
    SoftBrake,
    HardBrake,
}

impl Text {
    pub fn to_markdown(&self) -> String {
        match self {
            Self::Plain(txt) => txt.to_string(),
            Self::Italic(txt) => {
                let separator = "_";
                format!("{separator}{}{separator}", txt)
            }
            Self::Bold(txt) => {
                let separator = "**";
                format!("{separator}{}{separator}", txt)
            }
            Self::Strikethrough(txt) => {
                let separator = "~~";
                format!("{separator}{}{separator}", txt)
            }
            Self::SoftBrake => "\n".to_string(),
            Self::HardBrake => "\\\n".to_string(),
        }
    }
}

#[derive(Debug, Clone)]
pub enum NodeType {
    Document,
    Text(Text),
    Paragraph,
    Heading { level: usize, content: Vec<Text> },
}

#[derive(Debug, Clone)]
pub struct Node {
    node_type: NodeType,
    subnodes: Vec<Rc<Node>>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Tag {
    Italic,
    Bold,
    Strikethrough,
    Paragraph,
    Heading(usize),
}

impl Tag {
    fn from_start(tag: pulldown_cmark::Tag) -> Self {
        match tag {
            pulldown_cmark::Tag::Emphasis => Self::Italic,
            pulldown_cmark::Tag::Strong => Self::Bold,
            pulldown_cmark::Tag::Strikethrough => Self::Strikethrough,
            pulldown_cmark::Tag::Heading { level, .. } => {
                Self::Heading(level as usize)
            }
            pulldown_cmark::Tag::Paragraph => Self::Paragraph,
            _ => todo!(),
        }
    }

    fn from_end(tag_end: pulldown_cmark::TagEnd) -> Self {
        match tag_end {
            pulldown_cmark::TagEnd::Emphasis => Self::Italic,
            pulldown_cmark::TagEnd::Strong => Self::Bold,
            pulldown_cmark::TagEnd::Strikethrough => {
                Self::Strikethrough
            }
            _ => todo!(),
        }
    }
}

impl Node {
    fn parse_text_event(
        events: &mut dyn Iterator<Item = pulldown_cmark::Event>,
        tag: Tag,
    ) -> Result<Self, &'static str> {
        let txt_events = events
            .take_while(|event| match event {
                pulldown_cmark::Event::End(tag_end) => {
                    Tag::from_end(*tag_end) != tag
                }
                _ => true,
            })
            .collect::<Vec<_>>();
        let text_str = txt_events
            .iter()
            .filter_map(|event| match event {
                pulldown_cmark::Event::Text(txt) => {
                    Some(txt.to_string())
                }
                _ => None,
            })
            .collect::<Vec<_>>();
        if txt_events.len() != text_str.len() {
            return Err("Not all events are text events");
        }
        let text = text_str.join("");
        match tag {
            Tag::Italic => Ok(Self {
                node_type: NodeType::Text(Text::Italic(text)),
                subnodes: vec![],
            }),
            Tag::Bold => Ok(Self {
                node_type: NodeType::Text(Text::Bold(text)),
                subnodes: vec![],
            }),
            Tag::Strikethrough => Ok(Self {
                node_type: NodeType::Text(Text::Strikethrough(text)),
                subnodes: vec![],
            }),
            _ => Err("Not a text event"),
        }
    }

    fn parse_paragraph(
        events: &mut dyn Iterator<Item = pulldown_cmark::Event>,
    ) -> Result<Self, &'static str> {
        let text_events = events
            .take_while(|event| match event {
                pulldown_cmark::Event::End(tag_end) => {
                    Tag::from_end(*tag_end) != Tag::Paragraph
                }
                _ => true,
            })
            .collect::<Vec<_>>();
        let txt_nodes =
            Self::parse_nodes(&mut text_events.into_iter())?;

        for node in &txt_nodes {
            match node.node_type {
                NodeType::Text(_) => (),
                _ => return Err("Non text node was found"),
            }
        }

        Ok(Self {
            node_type: NodeType::Paragraph,
            subnodes: txt_nodes,
        })
    }

    fn parse_heading(
        events: &mut dyn Iterator<Item = pulldown_cmark::Event>,
        tag: Tag,
    ) -> Result<Self, &'static str> {
        let text_nodes = events.take_while(|event| match event {
            pulldown_cmark::Event::End(tag_end) => {
                Tag::from_end(*tag_end) != tag
            }
            _ => true,
        });
        let text_nodes =
            Self::parse_nodes(&mut text_nodes.into_iter())?;

        for node in &text_nodes {
            match node.node_type {
                NodeType::Text(_) => (),
                _ => return Err("Non text node was found"),
            }
        }

        let level = match tag {
            Tag::Heading(level) => Some(level),
            _ => None,
        }
        .ok_or("Not headinh tag")?;

        Ok(Self {
            node_type: NodeType::Heading {
                level,
                content: text_nodes
                    .iter()
                    .filter_map(|node| match &node.node_type {
                        NodeType::Text(txt) => Some(txt.clone()),
                        _ => None,
                    })
                    .collect(),
            },
            subnodes: vec![],
        })
    }

    fn parse_tag(
        events: &mut dyn Iterator<Item = pulldown_cmark::Event>,
        tag: Tag,
    ) -> Result<Self, &'static str> {
        match tag {
            Tag::Italic | Tag::Bold | Tag::Strikethrough => {
                Self::parse_text_event(events, tag)
            }
            Tag::Paragraph => Self::parse_paragraph(events),
            Tag::Heading(_) => Self::parse_heading(events, tag),
        }
    }

    pub fn parse_nodes(
        events: &mut dyn Iterator<Item = pulldown_cmark::Event>,
    ) -> Result<Vec<Rc<Self>>, &'static str> {
        let mut nodes = vec![];

        let mut open_headings = Vec::<Rc<Self>>::new();

        fn push_node(
            node: Node,
            mut nodes: Vec<Rc<Node>>,
            mut open_headings: Vec<Rc<Node>>,
        ) -> Result<(Vec<Rc<Node>>, Vec<Rc<Node>>), &'static str>
        {
            let push_level = match &node.node_type {
                NodeType::Heading { level, .. } => Some(*level),
                _ => None,
            };

            let rc_node = Rc::new(node);

            while let Some(n) = open_headings.last() {
                match &n.node_type {
                    NodeType::Heading { level: lvl, .. } => {
                        if *lvl >= push_level.unwrap_or(usize::MAX) {
                            open_headings.pop();
                        } else {
                            break;
                        }
                    }
                    _ => break,
                }
            }

            match open_headings.len() {
                0 => nodes.push(rc_node),
                size => Rc::get_mut(&mut open_headings[size - 1])
                    .ok_or("Cannot get mut from ref counted")?
                    .subnodes
                    .push(rc_node),
            };

            if push_level.is_some() {
                match open_headings.len() {
                    0usize => {
                        todo!();
                    }
                    _ => {
                        let size = open_headings.len();
                        let sub_size =
                            open_headings[size - 1].subnodes.len();
                        open_headings.push(
                            open_headings[size - 1].subnodes
                                [sub_size - 1]
                                .clone(),
                        )
                    }
                }
            }
            Ok((nodes, open_headings))
        }

        while let Some(event) = events.next() {
            let node = match event {
                pulldown_cmark::Event::Start(tag) => {
                    Self::parse_tag(events, Tag::from_start(tag))?
                }
                pulldown_cmark::Event::Text(txt) => Self {
                    node_type: NodeType::Text(Text::Plain(
                        txt.to_string(),
                    )),
                    subnodes: vec![],
                },
                _ => todo!(),
            };
            (nodes, open_headings) =
                push_node(node, nodes, open_headings)?;
        }

        Ok(nodes)
    }

    fn write_indented(
        &self,
        f: &mut std::fmt::Formatter<'_>,
        level: usize,
    ) -> std::fmt::Result {
        println!("{}", level);
        println!("{:#?}", self.node_type);
        match &self.node_type {
            NodeType::Document => {
                for node in &self.subnodes {
                    node.write_indented(f, level)?;
                }
            }
            NodeType::Text(txt) => {
                write!(
                    f,
                    "{:indent$}{}",
                    "",
                    txt.to_markdown(),
                    indent = level * 2
                )?;
            }
            NodeType::Heading {
                level: lvl,
                content,
            } => {
                write!(
                    f,
                    "{:indent$}{} {}",
                    "",
                    "#".repeat(*lvl),
                    content
                        .iter()
                        .map(|txt| txt.to_markdown())
                        .collect::<Vec<_>>()
                        .join(""),
                    indent = level * 2
                )?;
                writeln!(f)?;
                for node in &self.subnodes {
                    node.write_indented(f, level + 1)?;
                }
            }
            NodeType::Paragraph => {
                for node in &self.subnodes {
                    node.write_indented(f, level)?;
                }
            }
        }
        Ok(())
    }
}

impl std::fmt::Display for Node {
    fn fmt(
        &self,
        f: &mut std::fmt::Formatter<'_>,
    ) -> std::fmt::Result {
        self.write_indented(f, 0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::vec;

    #[test]
    fn test_write_indented() {
        let node = Node {
            node_type: NodeType::Document,
            subnodes: vec![Node {
                node_type: NodeType::Heading {
                    level: 1,
                    content: vec![Text::Plain("Heading".to_string())],
                },
                subnodes: vec![],
            }]
            .into_iter()
            .map(|n| Rc::new(n))
            .collect(),
        };

        let expected = "# Heading\n";

        let output = format!("{}", node);
        assert_eq!(output, expected);
    }
}
