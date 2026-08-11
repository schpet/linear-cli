# document

> Manage Linear documents

## Usage

```
Usage:   linear document

Description:

  Manage Linear documents

Options:

  -h, --help           - Show this help.                      
  --workspace  <slug>  - Target workspace (uses credentials)  

Commands:

  list, l                  - List documents                    
  view, v    <id>          - View a document's content         
  create, c                - Create a new document             
  update, u  <documentId>  - Update an existing document       
  delete, d  [documentId]  - Delete a document (moves to trash)
```

## Subcommands

### create

> Create a new document

```
Usage:   linear document create

Description:

  Create a new document

Options:

  -h, --help                        - Show this help.                                                                   
  --workspace         <slug>        - Target workspace (uses credentials)                                               
  -t, --title         <title>       - Document title (required)                                                         
  -c, --content       <content>     - Markdown content (inline)                                                         
  -f, --content-file  <path>        - Read content from file                                                            
  --project           <project>     - Attach to project (UUID, slug ID, or name)                                        
  --issue             <issue>       - Attach to issue (identifier like TC-123)                                          
  --initiative        <initiative>  - Attach to initiative (UUID, slug ID, or name)                                     
  --team              <team>        - Attach to team (key); with --cycle, scopes the cycle lookup instead               
  --cycle             <cycle>       - Attach to cycle: name, number, 'active'/'now', 'next', 'previous', or a relative  
                                      offset like +1 (team from --team or config)                                       
  --release           <release>     - Attach to release (UUID, name, or version)                                        
  --icon              <icon>        - Document icon (emoji)                                                             
  -i, --interactive                 - Interactive mode with prompts
```

### delete

> Delete a document (moves to trash)

```
Usage:   linear document delete [documentId]

Description:

  Delete a document (moves to trash)

Options:

  -h, --help              - Show this help.                                     
  --workspace   <slug>    - Target workspace (uses credentials)                 
  -y, --yes               - Skip confirmation prompt                            
  --bulk        <ids...>  - Delete multiple documents by slug or ID             
  --bulk-file   <file>    - Read document slugs/IDs from a file (one per line)  
  --bulk-stdin            - Read document slugs/IDs from stdin
```

### list

> List documents

```
Usage:   linear document list

Description:

  List documents

Options:

  -h, --help                  - Show this help.                                                                                
  --workspace   <slug>        - Target workspace (uses credentials)                                                            
  --project     <project>     - Filter by project (UUID, slug ID, or name)                                                     
  --issue       <issue>       - Filter by issue (identifier like TC-123)                                                       
  --initiative  <initiative>  - Filter by initiative (UUID, slug ID, or name)                                                  
  --team        <team>        - Filter by team (key); with --cycle, scopes the cycle lookup instead                            
  --cycle       <cycle>       - Filter by cycle: name, number, 'active'/'now', 'next', 'previous', or a relative               
                                offset like +1 (team from --team or config)                                                    
  --release     <release>     - Filter by release (UUID, name, or version)                                                     
  --json                      - Output as JSON                                                                                 
  --limit       <limit>       - Limit results                                                                     (Default: 50)
```

### update

> Update an existing document

```
Usage:   linear document update <documentId>

Description:

  Update an existing document

Options:

  -h, --help                        - Show this help.                                                                   
  --workspace         <slug>        - Target workspace (uses credentials)                                               
  -t, --title         <title>       - New title for the document                                                        
  -c, --content       <content>     - New markdown content (inline)                                                     
  -f, --content-file  <path>        - Read new content from file                                                        
  --icon              <icon>        - New icon (emoji)                                                                  
  --project           <project>     - Re-point to project (UUID, slug ID, or name); replaces the current attachment     
  --issue             <issue>       - Re-point to issue (identifier like TC-123); replaces the current attachment       
  --initiative        <initiative>  - Re-point to initiative (UUID, slug ID, or name); replaces the current attachment  
  --team              <team>        - Re-point to team (key); with --cycle, scopes the cycle lookup instead             
  --cycle             <cycle>       - Re-point to cycle: name, number, 'active'/'now', 'next', 'previous', or a         
                                      relative offset like +1 (team from --team or config)                              
  --release           <release>     - Re-point to release (UUID, name, or version); replaces the current attachment     
  -e, --edit                        - Open current content in $EDITOR for editing                                       
  --force                           - Update content even when document comments may lose inline anchors
```

### view

> View a document's content

```
Usage:   linear document view <id>

Description:

  View a document's content

Options:

  -h, --help             - Show this help.                                
  --workspace    <slug>  - Target workspace (uses credentials)            
  --raw                  - Output raw markdown without rendering          
  -w, --web              - Open document in browser                       
  --json                 - Output full document as JSON                   
  --no-download          - Keep remote URLs instead of downloading files
```
