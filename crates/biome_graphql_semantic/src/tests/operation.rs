// TODO: variables
use biome_graphql_parser::parse_graphql;
use biome_graphql_syntax::GraphqlArgument;
use biome_graphql_syntax::GraphqlVariableDefinition;

use crate::semantic_model;
use crate::HasDeclarationAstNode;
use crate::HasDeclarationAstNodes;

use super::assert_nodes_eq;
use super::extract_node;

#[test]
fn ok_variable() {
    let src = r#"
query ($storyId: ID = "1") {
	likeStory(storyId: $storyId)
}
"#;
    let parse_result = parse_graphql(src);
    let model = semantic_model(&parse_result.tree());
    let bindings = model.all_bindings().collect::<Vec<_>>();

    assert_nodes_eq(bindings, &["$storyId"]);

    let unresolved_references = model.all_unresolved_references().collect::<Vec<_>>();
    assert_nodes_eq(unresolved_references, &[]);

    let variable_definition = extract_node::<GraphqlVariableDefinition>(&parse_result);
    let argument = extract_node::<GraphqlArgument>(&parse_result);
    let variable_reference = argument.value().unwrap();
    let variable_reference = variable_reference.as_graphql_variable_reference().unwrap();
    let variable_binding = variable_reference.binding_nodes(&model);
    assert_eq!(
        variable_binding,
        vec![variable_definition.variable().unwrap()]
    );
}

#[test]
fn ok_variable_fragment() {
    let src = r#"
fragment StoryDetails on Story {
	likeStory(storyId: $storyId)
}
query ($storyId: ID = "1") {
    ...StoryDetails
}
"#;
    let parse_result = parse_graphql(src);
    let model = semantic_model(&parse_result.tree());
    let bindings = model.all_bindings().collect::<Vec<_>>();
    println!("{:?}", bindings);

    assert_nodes_eq(bindings, &["StoryDetails", "$storyId"]);

    let unresolved_references = model.all_unresolved_references().collect::<Vec<_>>();
    assert_nodes_eq(unresolved_references, &["Story"]);

    let variable_definition = extract_node::<GraphqlVariableDefinition>(&parse_result);
    let argument = extract_node::<GraphqlArgument>(&parse_result);
    let variable_reference = argument.value().unwrap();
    let variable_reference = variable_reference.as_graphql_variable_reference().unwrap();
    let variable_binding = variable_reference.binding_nodes(&model);
    assert_eq!(
        variable_binding,
        vec![variable_definition.variable().unwrap()]
    );
}
