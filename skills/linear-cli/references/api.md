# api

> Make a raw GraphQL API request

## Usage

```
Usage:   linear api [query]
Version: 1.9.1             

Description:

  Make a raw GraphQL API request

Options:

  -h, --help                    - Show this help.                                                                  
  -w, --workspace   <slug>      - Target workspace (uses credentials)                                              
  --variable        <variable>  - Variable in key=value format (coerces booleans, numbers, null; @file reads from  
                                  path)                                                                            
  --variables-json  <json>      - JSON object of variables (merged with --variable, which takes precedence)        
  --paginate                    - Auto-paginate a single connection field using cursor pagination                  
  --silent                      - Suppress response output (exit code still reflects errors)
```
