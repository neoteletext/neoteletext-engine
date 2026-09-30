<!--
Original TTI Form System specification, version 2.0, written for the jellytext project
(~/devel/jellytext/tti_form_system_spec.md, last modified 17 August 2025).
Copied unchanged into tellytxt.com on 1 October 2026 as a historical record.
It is superseded by the corrected version 3 in REQUIREMENTS.md Appendix C (decision E.31).
-->

# TTI Form System Complete Specification v2.0

## The Problem: Bridging Teletext and Modern Interactivity

### Historical Context

Teletext was a revolutionary broadcasting technology that delivered text-based information services directly to television screens. Its design philosophy was elegantly simple: **what you see is what you get**. Pages were "drawn" using characters, creating immediately understandable layouts that worked on any display.

However, teletext was inherently **read-only**. Users could navigate and view content, but couldn't input data or interact with forms. As computing evolved toward interactive web applications, this limitation became significant.

### The Challenge

Modern applications require interactive forms for:
- User registration and authentication
- E-commerce transactions  
- Survey and feedback collection
- Configuration and settings
- Multi-step workflows

But existing form technologies break teletext's core advantages:
- **HTML/CSS**: Requires complex rendering engines, breaks on simple terminals
- **GUI frameworks**: Need graphics capabilities, inaccessible to screen readers
- **Web forms**: Depend on JavaScript, fail in constrained environments

### The Vision

Create an interactive form system that preserves teletext's strengths:
- **Visual First**: Forms designed by literally drawing what users see
- **Universal Compatibility**: Works in terminals, embedded systems, accessibility devices
- **Backward Compatible**: Legacy teletext systems display readable (non-interactive) forms
- **Simple Syntax**: Minimal learning curve, maximum capability

---

## Design Philosophy

### Core Principles

**1. Visual First Design**
Forms are designed by drawing exactly what users see using text characters. No separation between design mockup and implementation.

**2. Position-Based Field Mapping**  
Interactive elements map to visual placeholders by their order of appearance, eliminating ambiguity.

**3. Proximity-Based Definitions**
Field definitions appear directly after their visual line, creating immediate visual connection between layout and behavior.

**4. Standard Form Submission**
Forms submit data to backend servers using standard HTTP methods and can redirect to success/error pages.

**5. Graceful Degradation**
Legacy systems that don't understand forms display readable static content.

---

## Core Syntax

### Visual Layout Commands
```
OL,line_number,content_with_visual_placeholders
```

### Visual Placeholders
- `_____` - Text input field (underscores show width)
- `( )` - Radio button (unselected)
- `(X)` - Radio button (selected)  
- `[ ]` - Checkbox (unchecked)
- `[X]` - Checkbox (checked)

### Field Definitions (Directly After Visual Line)
```
FD,type,name,parameters
```

### Form Configuration
```
FS,method,url                    # Form submission
FH,header_name,header_value      # HTTP headers  
FP,success_page,error_page       # Page routing
```

---

## Field Types & Parameters

### TEXT
`FD,TEXT,name,maxlen,required,pattern,default`

### EMAIL  
`FD,EMAIL,name,maxlen,required,default`

### TEL
`FD,TEL,name,maxlen,required,pattern,default`

### NUMBER
`FD,NUMBER,name,min,max,step,required,default`

### DATE
`FD,DATE,name,min_date,max_date,required,default`

### TIME
`FD,TIME,name,min_time,max_time,required,default`

### SELECT
`FD,SELECT,name,required`

### RADIO
`FD,RADIO,group_name,required`

### CHECKBOX
`FD,CHECKBOX,name,value,required`

### OPTION (for SELECT, RADIO)
`OPTION,value,display_text,selected`

### TEXTAREA
`FD,TEXTAREA,name,rows,maxlen,required,default`

### SUBMIT
`FD,SUBMIT,text,action`

### RESET
`FD,RESET,text`

---

## How It Works: Dual System Compatibility

### Legacy Teletext System Processing

Legacy systems only parse recognized teletext commands and ignore unknown commands like FD, OPTION, FS, etc.

**Step 1**: Parse only recognized commands
```
PN,40001
DE,Contact Form
PS,8000

OL,1,CONTACT US
OL,2,Name: _________________________________
OL,3,Email: ____________________________________

FD,TEXT,name,50,Y
FD,EMAIL,email,100,Y
```

**Step 2**: Display output
```
CONTACT US
Name: _________________________________
Email: ____________________________________
```

**Result**: A readable form showing labels and placeholder patterns, but no interactivity.

### Modern Form-Enabled System Processing

Modern systems parse all teletext commands and additionally process form-specific commands.

**Step 1**: Parse teletext commands normally
```
OL,1,CONTACT US
OL,2,Name: _________________________________
OL,3,Email: ____________________________________
```

**Step 2**: Process field definitions in context
```
OL,2,Name: _________________________________
FD,TEXT,name,50,Y

OL,3,Email: ____________________________________
FD,EMAIL,email,100,Y
```

**Step 3**: Map visual patterns to interactive elements

```
Line 2: "Name: _________________________________"
Pattern found: "________________________________" at position 6-40
Create: text input field with name="name", maxlength=50, required=true

Line 3: "Email: ____________________________________"  
Pattern found: "___________________________________" at position 7-43
Create: email input field with name="email", maxlength=100, required=true
```

**Result**: Interactive form with proper input validation and submission capability.

---

## Simple Example: Contact Form

```
PN,40001
DE,Contact Form
PS,8000

OL,1,CONTACT US
OL,2,========================================
OL,3,Name: _________________________________
FD,TEXT,name,50,Y
OL,4,Email: ____________________________________
FD,EMAIL,email,100,Y
OL,5,Phone: _________________ (optional)
FD,TEL,phone,20,N
OL,6,Subject: ______________________________
FD,TEXT,subject,100,Y
OL,7,Message:
OL,8,_______________________________________
OL,9,_______________________________________
OL,10,______________________________________
FD,TEXTAREA,message,3,500,Y
OL,11,Urgency: ( ) Low ( ) Medium ( ) High
FD,RADIO,urgency,Y
OPTION,low,Low
OPTION,medium,Medium,selected
OPTION,high,High
OL,12,[ SEND MESSAGE ]
FD,SUBMIT,Send Message,send_contact

FS,POST,https://api.company.com/contact
FH,Content-Type,application/x-www-form-urlencoded
FP,40101,40102
```

### How This Displays

**Legacy System:**
```
CONTACT US
========================================
Name: _________________________________
Email: ____________________________________
Phone: _________________ (optional)
Subject: ______________________________
Message:
_______________________________________
_______________________________________
______________________________________
Urgency: ( ) Low ( ) Medium ( ) High
[ SEND MESSAGE ]
```

**Modern System:**
```
CONTACT US
========================================
Name: [john.smith________________]
Email: [john@example.com___________]
Phone: [+44 123 456789___] (optional)
Subject: [Website feedback________]
Message:
[Great service, but the checkout  ]
[process could be simplified.     ]
[                                 ]
Urgency: (○) Low (●) Medium (○) High
[ SEND MESSAGE ]
```

---

## Complex Example: Multi-Field Lines

```
PN,40002
DE,Product Order
PS,8000

OL,1,ORDER FORM - Gaming Laptop
OL,2,========================================
OL,3,Customer Information:
OL,4,Name: _________________________________
FD,TEXT,customer_name,50,Y
OL,5,Email: ____________________________________
FD,EMAIL,customer_email,100,Y
OL,6,Product Configuration:
OL,7,Model: ____________ RAM: ____ Storage: ________
FD,SELECT,model,Y
OPTION,basic,Basic Gaming
OPTION,pro,Pro Gaming,selected
OPTION,ultimate,Ultimate Gaming
FD,SELECT,ram,Y
OPTION,16,16GB,selected
OPTION,32,32GB
OPTION,64,64GB
FD,SELECT,storage,Y
OPTION,512,512GB SSD
OPTION,1tb,1TB SSD,selected
OPTION,2tb,2TB SSD
OL,8,Color: ( ) Black ( ) Silver ( ) Red
FD,RADIO,color,Y
OPTION,black,Black,selected
OPTION,silver,Silver
OPTION,red,Red
OL,9,Warranty: [ ] Standard [ ] Extended (+£199)
FD,CHECKBOX,standard_warranty,included,N,selected
FD,CHECKBOX,extended_warranty,yes,N
OL,10,[ CALCULATE TOTAL ] [ PLACE ORDER ]
FD,SUBMIT,Calculate Total,calculate
FD,SUBMIT,Place Order,order

FS,POST,https://shop.company.com/api/orders
FH,Authorization,Bearer shop_token_xyz
FP,40301,40302
```

### Multi-Field Processing Logic

For lines with multiple fields, the system processes them sequentially from left to right.

**Line 7**: `Model: ____________ RAM: ____ Storage: ________`

**Pattern Detection:**
1. Scan for field patterns: `____________`, `____`, `________`
2. Map to field definitions in order:
   - Pattern 1 (____________) maps to SELECT model
   - Pattern 2 (____) maps to SELECT ram  
   - Pattern 3 (________) maps to SELECT storage

**Interactive Result:**
```
Model: [Pro Gaming ▼] RAM: [32GB▼] Storage: [1TB SSD▼]
```

---

## Backend Response Capability

After form submission, backends handle the data and redirect users:

### Response Handling
```
FP,success_page,error_page
```

Forms submit data to the specified URL and redirect to success or error pages based on the response.

---

## Advanced Features

### Error Handling
Server returns standard HTTP status codes. Error pages can be specified:
```
FP,success_page,error_page
```

### Multi-language Support
Field labels and messages can be localized by the server or client system.

### Form Validation
- **Client-side**: Basic type and required field validation
- **Server-side**: Full business logic validation with appropriate error responses

---

## Implementation Guidelines

### Field Pattern Recognition
Modern systems scan each OL line for recognizable patterns:
- **Text fields**: Sequences of underscores `___`
- **Radio buttons**: Parentheses patterns `( )` or `(X)`
- **Checkboxes**: Square bracket patterns `[ ]` or `[X]`
- **Buttons**: Square bracket text `[ SUBMIT ]`

### Position Mapping Algorithm
```
For each OL line:
  1. Scan left-to-right for field patterns
  2. Number patterns sequentially (1, 2, 3...)
  3. Read subsequent FD definitions until next OL
  4. Map FD definitions to patterns in order
  5. Create interactive elements at pattern positions
```

### Error Validation
- **Pattern count mismatch**: More/fewer FD lines than patterns found
- **Type incompatibility**: TEXT field defined for radio button pattern
- **Missing definitions**: Field patterns with no FD definitions

### Backward Compatibility Testing
All TTI forms should be tested on legacy teletext systems to ensure:
- All OL lines display correctly
- FD/OPTION lines are ignored gracefully  
- Forms remain readable and understandable
- No syntax errors or display corruption

---

## Benefits Summary

### For Developers
- **Rapid Prototyping**: Sketch forms directly in text editor
- **Zero Ambiguity**: Field definitions immediately follow visual layout
- **Easy Maintenance**: Adding/removing fields doesn't affect others
- **Universal Deployment**: Same source works for terminal, web, embedded systems

### For Users  
- **Accessibility**: Works perfectly with screen readers
- **Consistency**: Same visual approach across all platforms
- **Simplicity**: Clear, uncluttered interface design
- **Reliability**: Degrades gracefully on older systems

### For Organizations
- **Cost Effective**: No complex frontend frameworks required
- **Inclusive**: Reaches users with accessibility needs
- **Simple Integration**: Standard HTTP form submission to existing backends
- **Standards Based**: Builds on proven teletext infrastructure

---

## Migration Path

### Phase 1: Static Forms
Start with simple forms that display on legacy systems and collect data on modern systems.

### Phase 2: Interactive Elements
Add radio buttons, checkboxes, and select dropdowns for richer interactions.

### Phase 3: Backend Integration
Implement proper form submission, validation, and error handling with server-side processing.

### Phase 4: Advanced Features  
Add complex validation, multi-language support, and accessibility enhancements.

This specification provides effective form capability while maintaining teletext's core philosophy: elegant simplicity that works everywhere.