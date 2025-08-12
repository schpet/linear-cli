/* eslint-disable */
import * as types from './graphql';
import { TypedDocumentNode as DocumentNode } from '@graphql-typed-document-node/core';

/**
 * Map of all GraphQL operations in the project.
 *
 * This map has several performance disadvantages:
 * 1. It is not tree-shakeable, so it will include all operations in the project.
 * 2. It is not minifiable, so the string of a GraphQL query will be multiple times inside the bundle.
 * 3. It does not support dead code elimination, so it will add unused operations.
 *
 * Therefore it is highly recommended to use the babel or swc plugin for production.
 * Learn more about it here: https://the-guild.dev/graphql/codegen/plugins/presets/preset-client#reducing-bundle-size
 */
type Documents = {
    "\n    query GetStartedState($teamId: String!) {\n      team(id: $teamId) {\n        states {\n          nodes {\n            id\n            name\n            type\n            position\n          }\n        }\n      }\n    }\n  ": typeof types.GetStartedStateDocument,
    "\n    mutation UpdateIssueState($issueId: String!, $stateId: String!) {\n      issueUpdate(\n        id: $issueId,\n        input: { stateId: $stateId }\n      ) {\n        success\n      }\n    }\n  ": typeof types.UpdateIssueStateDocument,
    "\n    query GetProjectUidByName($name: String!) {\n      projects(filter: {name: {eq: $name}}) {nodes{id}}\n    }\n  ": typeof types.GetProjectUidByNameDocument,
    "\n    query GetProjectUidOptionsByName($name: String!) {\n        projects(filter: {name: {containsIgnoreCase: $name}}) {nodes{id, name}}\n      }\n  ": typeof types.GetProjectUidOptionsByNameDocument,
    "\n    query GetIssueIdByTitle($title: String!) {\n      issues(filter: {title: {eq: $title}}) {nodes{identifier}}\n    }\n  ": typeof types.GetIssueIdByTitleDocument,
    "\n    query GetIssueUidByIdentifier($identifier: String!) {\n      issue(id: $identifier) { id }\n    }\n  ": typeof types.GetIssueUidByIdentifierDocument,
    "\n    query GetIssueUidOptionsByTitle($title: String!) {\n        issues(filter: {title: {containsIgnoreCase: $title}}) {nodes{id, identifier, title}}\n      }\n  ": typeof types.GetIssueUidOptionsByTitleDocument,
    "\n    query GetTeamUidByKey($team: String!) {\n      teams(filter: {key: {eq: $team}}) {nodes{id}}\n    }\n  ": typeof types.GetTeamUidByKeyDocument,
    "\n    query GetTeamUidByName($team: String!) {\n      teams(filter: {name: {eq: $team}}) {nodes{id}}\n    }\n  ": typeof types.GetTeamUidByNameDocument,
    "\n    query GetTeamUidOptionsByKey($team: String!) {\n        teams(filter: {key: {containsIgnoreCase: $team}}) {nodes{id, key}}\n      }\n  ": typeof types.GetTeamUidOptionsByKeyDocument,
    "\n    query GetTeamUidOptionsByName($team: String!) {\n        teams(filter: {name: {containsIgnoreCase: $team}}) {nodes{id, key, name}}\n      }\n  ": typeof types.GetTeamUidOptionsByNameDocument,
    "\n    query GetUserUidByDisplayName($username: String!) {\n      users(filter: {displayName: {eq: $username}}) {nodes{id}}\n    }\n  ": typeof types.GetUserUidByDisplayNameDocument,
    "\n    query GetUserUidOptionsByDisplayName($name: String!) {\n        users(filter: {displayName: {containsIgnoreCase: $name}}) {nodes{id, displayName}}\n      }\n  ": typeof types.GetUserUidOptionsByDisplayNameDocument,
    "\n    query GetUserUidOptionsByName($name: String!) {\n        users(filter: {name: {containsIgnoreCase: $name}}) {nodes{id, name}}\n      }\n  ": typeof types.GetUserUidOptionsByNameDocument,
    "\n      query GetViewerId {\n      viewer {id}\n    }\n    ": typeof types.GetViewerIdDocument,
    "\n    query GetIssueLabelUidByName($name: String!) {\n      issueLabels(filter: {name: {eq: $name}}) {nodes{id}}\n    }\n  ": typeof types.GetIssueLabelUidByNameDocument,
    "\n    query GetIssueLabelUidOptionsByName($name: String!) {\n        issueLabels(filter: {name: {containsIgnoreCase: $name}}) {nodes{id, name}}\n      }\n  ": typeof types.GetIssueLabelUidOptionsByNameDocument,
    "\n    query GetIssuesForState($teamId: String!, $sort: [IssueSortInput!], $states: [String!]) {\n      issues(\n        filter: {\n          team: { key: { eq: $teamId } }\n          assignee: { isMe: { eq: true } }\n          state: { type: { in: $states } }\n        }\n        sort: $sort\n      ) {\n        nodes {\n          id\n          identifier\n          title\n          priority\n          estimate\n          state {\n            id\n            name\n            color\n          }\n          labels {\n            nodes {\n              id\n              name\n              color\n            }\n          }\n          updatedAt\n        }\n      }\n    }\n  ": typeof types.GetIssuesForStateDocument,
    "\n      query GetIssueDetails($id: String!) {\n        issue(id: $id) { title, description, url, branchName }\n      }\n    ": typeof types.GetIssueDetailsDocument,
    "\n          mutation CreateIssue($input: IssueCreateInput!) {\n            issueCreate(input: $input) {\n              success\n              issue { id, team { key } }\n            }\n          }\n        ": typeof types.CreateIssueDocument,
    "\n  query Config {\n    viewer {\n      organization {\n        urlKey\n      }\n    }\n    teams {\n      nodes {\n        id\n        key\n        name\n      }\n    }\n  }\n": typeof types.ConfigDocument,
};
const documents: Documents = {
    "\n    query GetStartedState($teamId: String!) {\n      team(id: $teamId) {\n        states {\n          nodes {\n            id\n            name\n            type\n            position\n          }\n        }\n      }\n    }\n  ": types.GetStartedStateDocument,
    "\n    mutation UpdateIssueState($issueId: String!, $stateId: String!) {\n      issueUpdate(\n        id: $issueId,\n        input: { stateId: $stateId }\n      ) {\n        success\n      }\n    }\n  ": types.UpdateIssueStateDocument,
    "\n    query GetProjectUidByName($name: String!) {\n      projects(filter: {name: {eq: $name}}) {nodes{id}}\n    }\n  ": types.GetProjectUidByNameDocument,
    "\n    query GetProjectUidOptionsByName($name: String!) {\n        projects(filter: {name: {containsIgnoreCase: $name}}) {nodes{id, name}}\n      }\n  ": types.GetProjectUidOptionsByNameDocument,
    "\n    query GetIssueIdByTitle($title: String!) {\n      issues(filter: {title: {eq: $title}}) {nodes{identifier}}\n    }\n  ": types.GetIssueIdByTitleDocument,
    "\n    query GetIssueUidByIdentifier($identifier: String!) {\n      issue(id: $identifier) { id }\n    }\n  ": types.GetIssueUidByIdentifierDocument,
    "\n    query GetIssueUidOptionsByTitle($title: String!) {\n        issues(filter: {title: {containsIgnoreCase: $title}}) {nodes{id, identifier, title}}\n      }\n  ": types.GetIssueUidOptionsByTitleDocument,
    "\n    query GetTeamUidByKey($team: String!) {\n      teams(filter: {key: {eq: $team}}) {nodes{id}}\n    }\n  ": types.GetTeamUidByKeyDocument,
    "\n    query GetTeamUidByName($team: String!) {\n      teams(filter: {name: {eq: $team}}) {nodes{id}}\n    }\n  ": types.GetTeamUidByNameDocument,
    "\n    query GetTeamUidOptionsByKey($team: String!) {\n        teams(filter: {key: {containsIgnoreCase: $team}}) {nodes{id, key}}\n      }\n  ": types.GetTeamUidOptionsByKeyDocument,
    "\n    query GetTeamUidOptionsByName($team: String!) {\n        teams(filter: {name: {containsIgnoreCase: $team}}) {nodes{id, key, name}}\n      }\n  ": types.GetTeamUidOptionsByNameDocument,
    "\n    query GetUserUidByDisplayName($username: String!) {\n      users(filter: {displayName: {eq: $username}}) {nodes{id}}\n    }\n  ": types.GetUserUidByDisplayNameDocument,
    "\n    query GetUserUidOptionsByDisplayName($name: String!) {\n        users(filter: {displayName: {containsIgnoreCase: $name}}) {nodes{id, displayName}}\n      }\n  ": types.GetUserUidOptionsByDisplayNameDocument,
    "\n    query GetUserUidOptionsByName($name: String!) {\n        users(filter: {name: {containsIgnoreCase: $name}}) {nodes{id, name}}\n      }\n  ": types.GetUserUidOptionsByNameDocument,
    "\n      query GetViewerId {\n      viewer {id}\n    }\n    ": types.GetViewerIdDocument,
    "\n    query GetIssueLabelUidByName($name: String!) {\n      issueLabels(filter: {name: {eq: $name}}) {nodes{id}}\n    }\n  ": types.GetIssueLabelUidByNameDocument,
    "\n    query GetIssueLabelUidOptionsByName($name: String!) {\n        issueLabels(filter: {name: {containsIgnoreCase: $name}}) {nodes{id, name}}\n      }\n  ": types.GetIssueLabelUidOptionsByNameDocument,
    "\n    query GetIssuesForState($teamId: String!, $sort: [IssueSortInput!], $states: [String!]) {\n      issues(\n        filter: {\n          team: { key: { eq: $teamId } }\n          assignee: { isMe: { eq: true } }\n          state: { type: { in: $states } }\n        }\n        sort: $sort\n      ) {\n        nodes {\n          id\n          identifier\n          title\n          priority\n          estimate\n          state {\n            id\n            name\n            color\n          }\n          labels {\n            nodes {\n              id\n              name\n              color\n            }\n          }\n          updatedAt\n        }\n      }\n    }\n  ": types.GetIssuesForStateDocument,
    "\n      query GetIssueDetails($id: String!) {\n        issue(id: $id) { title, description, url, branchName }\n      }\n    ": types.GetIssueDetailsDocument,
    "\n          mutation CreateIssue($input: IssueCreateInput!) {\n            issueCreate(input: $input) {\n              success\n              issue { id, team { key } }\n            }\n          }\n        ": types.CreateIssueDocument,
    "\n  query Config {\n    viewer {\n      organization {\n        urlKey\n      }\n    }\n    teams {\n      nodes {\n        id\n        key\n        name\n      }\n    }\n  }\n": types.ConfigDocument,
};

/**
 * The gql function is used to parse GraphQL queries into a document that can be used by GraphQL clients.
 *
 *
 * @example
 * ```ts
 * const query = gql(`query GetUser($id: ID!) { user(id: $id) { name } }`);
 * ```
 *
 * The query argument is unknown!
 * Please regenerate the types.
 */
export function gql(source: string): unknown;

/**
 * The gql function is used to parse GraphQL queries into a document that can be used by GraphQL clients.
 */
export function gql(source: "\n    query GetStartedState($teamId: String!) {\n      team(id: $teamId) {\n        states {\n          nodes {\n            id\n            name\n            type\n            position\n          }\n        }\n      }\n    }\n  "): (typeof documents)["\n    query GetStartedState($teamId: String!) {\n      team(id: $teamId) {\n        states {\n          nodes {\n            id\n            name\n            type\n            position\n          }\n        }\n      }\n    }\n  "];
/**
 * The gql function is used to parse GraphQL queries into a document that can be used by GraphQL clients.
 */
export function gql(source: "\n    mutation UpdateIssueState($issueId: String!, $stateId: String!) {\n      issueUpdate(\n        id: $issueId,\n        input: { stateId: $stateId }\n      ) {\n        success\n      }\n    }\n  "): (typeof documents)["\n    mutation UpdateIssueState($issueId: String!, $stateId: String!) {\n      issueUpdate(\n        id: $issueId,\n        input: { stateId: $stateId }\n      ) {\n        success\n      }\n    }\n  "];
/**
 * The gql function is used to parse GraphQL queries into a document that can be used by GraphQL clients.
 */
export function gql(source: "\n    query GetProjectUidByName($name: String!) {\n      projects(filter: {name: {eq: $name}}) {nodes{id}}\n    }\n  "): (typeof documents)["\n    query GetProjectUidByName($name: String!) {\n      projects(filter: {name: {eq: $name}}) {nodes{id}}\n    }\n  "];
/**
 * The gql function is used to parse GraphQL queries into a document that can be used by GraphQL clients.
 */
export function gql(source: "\n    query GetProjectUidOptionsByName($name: String!) {\n        projects(filter: {name: {containsIgnoreCase: $name}}) {nodes{id, name}}\n      }\n  "): (typeof documents)["\n    query GetProjectUidOptionsByName($name: String!) {\n        projects(filter: {name: {containsIgnoreCase: $name}}) {nodes{id, name}}\n      }\n  "];
/**
 * The gql function is used to parse GraphQL queries into a document that can be used by GraphQL clients.
 */
export function gql(source: "\n    query GetIssueIdByTitle($title: String!) {\n      issues(filter: {title: {eq: $title}}) {nodes{identifier}}\n    }\n  "): (typeof documents)["\n    query GetIssueIdByTitle($title: String!) {\n      issues(filter: {title: {eq: $title}}) {nodes{identifier}}\n    }\n  "];
/**
 * The gql function is used to parse GraphQL queries into a document that can be used by GraphQL clients.
 */
export function gql(source: "\n    query GetIssueUidByIdentifier($identifier: String!) {\n      issue(id: $identifier) { id }\n    }\n  "): (typeof documents)["\n    query GetIssueUidByIdentifier($identifier: String!) {\n      issue(id: $identifier) { id }\n    }\n  "];
/**
 * The gql function is used to parse GraphQL queries into a document that can be used by GraphQL clients.
 */
export function gql(source: "\n    query GetIssueUidOptionsByTitle($title: String!) {\n        issues(filter: {title: {containsIgnoreCase: $title}}) {nodes{id, identifier, title}}\n      }\n  "): (typeof documents)["\n    query GetIssueUidOptionsByTitle($title: String!) {\n        issues(filter: {title: {containsIgnoreCase: $title}}) {nodes{id, identifier, title}}\n      }\n  "];
/**
 * The gql function is used to parse GraphQL queries into a document that can be used by GraphQL clients.
 */
export function gql(source: "\n    query GetTeamUidByKey($team: String!) {\n      teams(filter: {key: {eq: $team}}) {nodes{id}}\n    }\n  "): (typeof documents)["\n    query GetTeamUidByKey($team: String!) {\n      teams(filter: {key: {eq: $team}}) {nodes{id}}\n    }\n  "];
/**
 * The gql function is used to parse GraphQL queries into a document that can be used by GraphQL clients.
 */
export function gql(source: "\n    query GetTeamUidByName($team: String!) {\n      teams(filter: {name: {eq: $team}}) {nodes{id}}\n    }\n  "): (typeof documents)["\n    query GetTeamUidByName($team: String!) {\n      teams(filter: {name: {eq: $team}}) {nodes{id}}\n    }\n  "];
/**
 * The gql function is used to parse GraphQL queries into a document that can be used by GraphQL clients.
 */
export function gql(source: "\n    query GetTeamUidOptionsByKey($team: String!) {\n        teams(filter: {key: {containsIgnoreCase: $team}}) {nodes{id, key}}\n      }\n  "): (typeof documents)["\n    query GetTeamUidOptionsByKey($team: String!) {\n        teams(filter: {key: {containsIgnoreCase: $team}}) {nodes{id, key}}\n      }\n  "];
/**
 * The gql function is used to parse GraphQL queries into a document that can be used by GraphQL clients.
 */
export function gql(source: "\n    query GetTeamUidOptionsByName($team: String!) {\n        teams(filter: {name: {containsIgnoreCase: $team}}) {nodes{id, key, name}}\n      }\n  "): (typeof documents)["\n    query GetTeamUidOptionsByName($team: String!) {\n        teams(filter: {name: {containsIgnoreCase: $team}}) {nodes{id, key, name}}\n      }\n  "];
/**
 * The gql function is used to parse GraphQL queries into a document that can be used by GraphQL clients.
 */
export function gql(source: "\n    query GetUserUidByDisplayName($username: String!) {\n      users(filter: {displayName: {eq: $username}}) {nodes{id}}\n    }\n  "): (typeof documents)["\n    query GetUserUidByDisplayName($username: String!) {\n      users(filter: {displayName: {eq: $username}}) {nodes{id}}\n    }\n  "];
/**
 * The gql function is used to parse GraphQL queries into a document that can be used by GraphQL clients.
 */
export function gql(source: "\n    query GetUserUidOptionsByDisplayName($name: String!) {\n        users(filter: {displayName: {containsIgnoreCase: $name}}) {nodes{id, displayName}}\n      }\n  "): (typeof documents)["\n    query GetUserUidOptionsByDisplayName($name: String!) {\n        users(filter: {displayName: {containsIgnoreCase: $name}}) {nodes{id, displayName}}\n      }\n  "];
/**
 * The gql function is used to parse GraphQL queries into a document that can be used by GraphQL clients.
 */
export function gql(source: "\n    query GetUserUidOptionsByName($name: String!) {\n        users(filter: {name: {containsIgnoreCase: $name}}) {nodes{id, name}}\n      }\n  "): (typeof documents)["\n    query GetUserUidOptionsByName($name: String!) {\n        users(filter: {name: {containsIgnoreCase: $name}}) {nodes{id, name}}\n      }\n  "];
/**
 * The gql function is used to parse GraphQL queries into a document that can be used by GraphQL clients.
 */
export function gql(source: "\n      query GetViewerId {\n      viewer {id}\n    }\n    "): (typeof documents)["\n      query GetViewerId {\n      viewer {id}\n    }\n    "];
/**
 * The gql function is used to parse GraphQL queries into a document that can be used by GraphQL clients.
 */
export function gql(source: "\n    query GetIssueLabelUidByName($name: String!) {\n      issueLabels(filter: {name: {eq: $name}}) {nodes{id}}\n    }\n  "): (typeof documents)["\n    query GetIssueLabelUidByName($name: String!) {\n      issueLabels(filter: {name: {eq: $name}}) {nodes{id}}\n    }\n  "];
/**
 * The gql function is used to parse GraphQL queries into a document that can be used by GraphQL clients.
 */
export function gql(source: "\n    query GetIssueLabelUidOptionsByName($name: String!) {\n        issueLabels(filter: {name: {containsIgnoreCase: $name}}) {nodes{id, name}}\n      }\n  "): (typeof documents)["\n    query GetIssueLabelUidOptionsByName($name: String!) {\n        issueLabels(filter: {name: {containsIgnoreCase: $name}}) {nodes{id, name}}\n      }\n  "];
/**
 * The gql function is used to parse GraphQL queries into a document that can be used by GraphQL clients.
 */
export function gql(source: "\n    query GetIssuesForState($teamId: String!, $sort: [IssueSortInput!], $states: [String!]) {\n      issues(\n        filter: {\n          team: { key: { eq: $teamId } }\n          assignee: { isMe: { eq: true } }\n          state: { type: { in: $states } }\n        }\n        sort: $sort\n      ) {\n        nodes {\n          id\n          identifier\n          title\n          priority\n          estimate\n          state {\n            id\n            name\n            color\n          }\n          labels {\n            nodes {\n              id\n              name\n              color\n            }\n          }\n          updatedAt\n        }\n      }\n    }\n  "): (typeof documents)["\n    query GetIssuesForState($teamId: String!, $sort: [IssueSortInput!], $states: [String!]) {\n      issues(\n        filter: {\n          team: { key: { eq: $teamId } }\n          assignee: { isMe: { eq: true } }\n          state: { type: { in: $states } }\n        }\n        sort: $sort\n      ) {\n        nodes {\n          id\n          identifier\n          title\n          priority\n          estimate\n          state {\n            id\n            name\n            color\n          }\n          labels {\n            nodes {\n              id\n              name\n              color\n            }\n          }\n          updatedAt\n        }\n      }\n    }\n  "];
/**
 * The gql function is used to parse GraphQL queries into a document that can be used by GraphQL clients.
 */
export function gql(source: "\n      query GetIssueDetails($id: String!) {\n        issue(id: $id) { title, description, url, branchName }\n      }\n    "): (typeof documents)["\n      query GetIssueDetails($id: String!) {\n        issue(id: $id) { title, description, url, branchName }\n      }\n    "];
/**
 * The gql function is used to parse GraphQL queries into a document that can be used by GraphQL clients.
 */
export function gql(source: "\n          mutation CreateIssue($input: IssueCreateInput!) {\n            issueCreate(input: $input) {\n              success\n              issue { id, team { key } }\n            }\n          }\n        "): (typeof documents)["\n          mutation CreateIssue($input: IssueCreateInput!) {\n            issueCreate(input: $input) {\n              success\n              issue { id, team { key } }\n            }\n          }\n        "];
/**
 * The gql function is used to parse GraphQL queries into a document that can be used by GraphQL clients.
 */
export function gql(source: "\n  query Config {\n    viewer {\n      organization {\n        urlKey\n      }\n    }\n    teams {\n      nodes {\n        id\n        key\n        name\n      }\n    }\n  }\n"): (typeof documents)["\n  query Config {\n    viewer {\n      organization {\n        urlKey\n      }\n    }\n    teams {\n      nodes {\n        id\n        key\n        name\n      }\n    }\n  }\n"];

export function gql(source: string) {
  return (documents as any)[source] ?? {};
}

export type DocumentType<TDocumentNode extends DocumentNode<any, any>> = TDocumentNode extends DocumentNode<  infer TType,  any>  ? TType  : never;