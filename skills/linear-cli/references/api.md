# api

> Make a raw GraphQL API request

## Usage

```
Usage:   linear api [graphqlDocument]

Description:

  Make a raw GraphQL API request                                                                                                       
                                                                                                                                       
  Pass the GraphQL document as one quoted argument or on stdin. The api command has no subcommands: a leading query or mutation keyword
  belongs inside that document.                                                                                                        

Options:

  -h, --help                    - Show this help.                                                                  
  --workspace       <slug>      - Target workspace (uses credentials)                                              
  --variable        <variable>  - Variable in key=value format (coerces booleans, numbers, null; @file reads from  
                                  path)                                                                            
  --variables-json  <json>      - JSON object of variables (merged with --variable, which takes precedence)        
  --paginate                    - Auto-paginate a single connection field using cursor pagination                  
  --silent                      - Suppress response output (exit code still reflects errors)                       

Examples:

  Run an inline document           linear api '{ viewer { id name } }'                                                                                                           
  Run a named query with variables linear api 'query RecentIssues($first: Int) { issues(first: $first) { nodes { identifier title } } }' --variable first=5                      
  Pipe a document from stdin       echo '{ viewer { id } }' | linear api                                                                                                         
  Read a document from a file      linear api - < issues.graphql                                                                                                                 
  Auto-paginate a connection       linear api --paginate 'query($after: String) { issues(first: 50, after: $after) { nodes { identifier } pageInfo { hasNextPage endCursor } } }'
```
