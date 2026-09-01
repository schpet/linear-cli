# markdown

> Linear-flavored Markdown: mentions and collapsible sections

## Usage

```
Usage:   linear markdown

Description:

  Linear-flavored Markdown: mentions and collapsible sections                  
                                                                               
  These rules apply to comment bodies, issue descriptions, document content,   
  project overviews, and status update bodies.                                 
                                                                               
  MENTIONS                                                                     
                                                                               
  A resource's plain Linear URL becomes a linked mention. A literal `@name`, an
  `@[Name](id)`, or a Markdown link such as `[Name](url)` does not — it stays  
  plain text and notifies nobody. Put the bare URL in the body:                
                                                                               
  https://linear.app/acme/profiles/someuser can you take a look?               
                                                                               
  RESOLVING PEOPLE                                                             
                                                                               
  Look the person up in the relevant team first. The team can usually be       
  inferred from the issue identifier or the current directory:                 
                                                                               
  linear team members ENG --json                                               
                                                                               
  Paste the selected member's `url` field verbatim. If the intended person is  
  not a member of that team, stop and confirm before searching the whole       
  workspace with `linear user list --json`; mentioning someone outside the team
  is likely accidental.                                                        
                                                                               
  To mention an issue, use its URL the same way:                               
                                                                               
  linear issue url ENG-123                                                     
                                                                               
  COLLAPSIBLE SECTIONS                                                         
                                                                               
  Open a section with `+++ [title]` and close it with `+++`:                   
                                                                               
  +++ [Server log]                                                             
                                                                               
  Markdown content that is initially hidden.                                   
                                                                               
  +++                                                                          
                                                                               
  The square brackets around the title and the closing `+++` are both required.

Options:

  -h, --help           - Show this help.                      
  --workspace  <slug>  - Target workspace (uses credentials)
```
