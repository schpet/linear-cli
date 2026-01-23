# document

> Manage Linear documents

## Usage

```
Usage:   linear document
Version: 1.8.1          

Description:

  Manage Linear documents

Options:

  -h, --help               - Show this help.                      
  -w, --workspace  <slug>  - Target workspace (uses credentials)  

Commands:

  list, l                  - List documents                    
  view, v    <id>          - View a document's content         
  create, c                - Create a new document             
  update, u  <documentId>  - Update an existing document       
  delete, d  [documentId]  - Delete a document (moves to trash)
```

## Subcommands

### list

> List documents

```
Usage:   linear document list
Version: 1.8.1               

Description:

  List documents

Options:

  -h, --help                  - Show this help.                                        
  -w, --workspace  <slug>     - Target workspace (uses credentials)                    
  --project        <project>  - Filter by project (slug or name)                       
  --issue          <issue>    - Filter by issue (identifier like TC-123)               
  --json                      - Output as JSON                                         
  --limit          <limit>    - Limit results                             (Default: 50)
```

### view

> View a document's content

```
Usage:   linear document view <id>
Version: 1.8.1                    

Description:

  View a document's content

Options:

  -h, --help               - Show this help.                        
  -w, --workspace  <slug>  - Target workspace (uses credentials)    
  --raw                    - Output raw markdown without rendering  
  -w, --web                - Open document in browser               
  --json                   - Output full document as JSON
```

### create

> Create a new document

```
Usage:   linear document create
Version: 1.8.1                 

Description:

  Create a new document

Options:

  -h, --help                     - Show this help.                           
  -w, --workspace     <slug>     - Target workspace (uses credentials)       
  -t, --title         <title>    - Document title (required)                 
  -c, --content       <content>  - Markdown content (inline)                 
  -f, --content-file  <path>     - Read content from file                    
  --project           <project>  - Attach to project (slug or ID)            
  --issue             <issue>    - Attach to issue (identifier like TC-123)  
  --icon              <icon>     - Document icon (emoji)                     
  -i, --interactive              - Interactive mode with prompts             
  --no-color                     - Disable colored output
```

### update

> Update an existing document

```
Usage:   linear document update <documentId>
Version: 1.8.1                              

Description:

  Update an existing document

Options:

  -h, --help                     - Show this help.                              
  -w, --workspace     <slug>     - Target workspace (uses credentials)          
  -t, --title         <title>    - New title for the document                   
  -c, --content       <content>  - New markdown content (inline)                
  -f, --content-file  <path>     - Read new content from file                   
  --icon              <icon>     - New icon (emoji)                             
  -e, --edit                     - Open current content in $EDITOR for editing  
  --no-color                     - Disable colored output
```

### delete

> Delete a document (moves to trash)

```
Usage:   linear document delete [documentId]
Version: 1.8.1                              

Description:

  Delete a document (moves to trash)

Options:

  -h, --help                 - Show this help.                                     
  -w, --workspace  <slug>    - Target workspace (uses credentials)                 
  -y, --yes                  - Skip confirmation prompt                            
  --no-color                 - Disable colored output                              
  --bulk           <ids...>  - Delete multiple documents by slug or ID             
  --bulk-file      <file>    - Read document slugs/IDs from a file (one per line)  
  --bulk-stdin               - Read document slugs/IDs from stdin
```
