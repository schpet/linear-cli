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
    "\n    query issues($teamId: String!, $sort: [IssueSortInput!], $states: [String!]) {\n      issues(\n        filter: {\n          team: { key: { eq: $teamId } }\n          assignee: { isMe: { eq: true } }\n          state: { type: { in: $states } }\n        }\n        sort: $sort\n      ) {\n        nodes {\n          id\n          identifier\n          title\n          priority\n          estimate\n          state {\n            id\n            name\n            color\n          }\n          labels {\n            nodes {\n              id\n              name\n              color\n            }\n          }\n          updatedAt\n        }\n      }\n    }\n  ": typeof types.IssuesDocument,
    "\n  query Config {\n    viewer {\n      organization {\n        urlKey\n      }\n    }\n    teams {\n      nodes {\n        id\n        key\n        name\n      }\n    }\n  }\n": typeof types.ConfigDocument,
};
const documents: Documents = {
    "\n    query GetStartedState($teamId: String!) {\n      team(id: $teamId) {\n        states {\n          nodes {\n            id\n            name\n            type\n            position\n          }\n        }\n      }\n    }\n  ": types.GetStartedStateDocument,
    "\n    query issues($teamId: String!, $sort: [IssueSortInput!], $states: [String!]) {\n      issues(\n        filter: {\n          team: { key: { eq: $teamId } }\n          assignee: { isMe: { eq: true } }\n          state: { type: { in: $states } }\n        }\n        sort: $sort\n      ) {\n        nodes {\n          id\n          identifier\n          title\n          priority\n          estimate\n          state {\n            id\n            name\n            color\n          }\n          labels {\n            nodes {\n              id\n              name\n              color\n            }\n          }\n          updatedAt\n        }\n      }\n    }\n  ": types.IssuesDocument,
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
export function gql(source: "\n    query issues($teamId: String!, $sort: [IssueSortInput!], $states: [String!]) {\n      issues(\n        filter: {\n          team: { key: { eq: $teamId } }\n          assignee: { isMe: { eq: true } }\n          state: { type: { in: $states } }\n        }\n        sort: $sort\n      ) {\n        nodes {\n          id\n          identifier\n          title\n          priority\n          estimate\n          state {\n            id\n            name\n            color\n          }\n          labels {\n            nodes {\n              id\n              name\n              color\n            }\n          }\n          updatedAt\n        }\n      }\n    }\n  "): (typeof documents)["\n    query issues($teamId: String!, $sort: [IssueSortInput!], $states: [String!]) {\n      issues(\n        filter: {\n          team: { key: { eq: $teamId } }\n          assignee: { isMe: { eq: true } }\n          state: { type: { in: $states } }\n        }\n        sort: $sort\n      ) {\n        nodes {\n          id\n          identifier\n          title\n          priority\n          estimate\n          state {\n            id\n            name\n            color\n          }\n          labels {\n            nodes {\n              id\n              name\n              color\n            }\n          }\n          updatedAt\n        }\n      }\n    }\n  "];
/**
 * The gql function is used to parse GraphQL queries into a document that can be used by GraphQL clients.
 */
export function gql(source: "\n  query Config {\n    viewer {\n      organization {\n        urlKey\n      }\n    }\n    teams {\n      nodes {\n        id\n        key\n        name\n      }\n    }\n  }\n"): (typeof documents)["\n  query Config {\n    viewer {\n      organization {\n        urlKey\n      }\n    }\n    teams {\n      nodes {\n        id\n        key\n        name\n      }\n    }\n  }\n"];

export function gql(source: string) {
  return (documents as any)[source] ?? {};
}

export type DocumentType<TDocumentNode extends DocumentNode<any, any>> = TDocumentNode extends DocumentNode<  infer TType,  any>  ? TType  : never;