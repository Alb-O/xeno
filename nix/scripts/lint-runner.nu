#!/usr/bin/env nu
# Unified lint runner: ast-grep rules + custom rules (clippy, file metrics)
# Run from project root. Use --json for machine-readable output.

def make-finding [
    rule_id: string
    severity: string
    message: string
    file: string
    start_line: int
    start_col: int
    end_line: int
    end_col: int
    text: string
]: nothing -> record {
    {
        ruleId: $rule_id
        severity: $severity
        message: $message
        file: $file
        range: {
            start: { line: $start_line, column: $start_col }
            end: { line: $end_line, column: $end_col }
        }
        text: $text
    }
}

def run-ast-grep [rules_dir?: string]: nothing -> list {
    if (which ast-grep | is-empty) { return [] }

    # Use provided rules dir, or fall back to LINTFRA_RULES env, or default location
    let dir = $rules_dir | default ($env.LINTFRA_RULES? | default "lint/ast-rules")
    
    let result = if ($dir | path exists) {
        ^ast-grep scan --rule $dir --json=stream | complete
    } else {
        ^ast-grep scan --json=stream | complete
    }
    
    if $result.exit_code in [0, 1] {
        $result.stdout | lines | where { $in | str trim | is-not-empty } | each { $in | from json }
    } else {
        []
    }
}

def parse-clippy-output []: string -> list {
    $in
    | lines
    | where { $in | str starts-with "{" }
    | each { $in | from json }
    | where { ($in.reason? == "compiler-message") and ($in.message?.level? != "note") }
    | each {|msg|
        let m = $msg.message
        $m.spans?
        | default []
        | where { $in.is_primary? == true }
        | each {|span|
            let text = ($span.text? | default [{}]) | get 0.text? | default "" | str trim
            make-finding ($m.code?.code? | default "clippy") $m.level $m.message $span.file_name (($span.line_start? | default 1) - 1) (($span.column_start? | default 1) - 1) (($span.line_end? | default 1) - 1) (($span.column_end? | default 1) - 1) $text
        }
    }
    | flatten
}

def parse-vimgrep-output [
    rule_id: string
    severity: string
    message: string
]: string -> list {
    $in
    | lines
    | where { $in | str trim | is-not-empty }
    | each {|line|
        let parts = ($line | split row ":")
        if ($parts | length) < 4 {
            null
        } else {
            let file = ($parts | get 0)
            let line_no = (($parts | get 1 | into int) - 1)
            let col_no = (($parts | get 2 | into int) - 1)
            let text = ($parts | skip 3 | str join ":")
            make-finding $rule_id $severity $message $file $line_no $col_no $line_no $col_no $text
        }
    }
    | compact
}

def run-command-rule [rule: record, project_root: string]: nothing -> list {
    let run_cmd = ($rule.run? | default "")
    if ($run_cmd | is-empty) { return [] }

    cd $project_root
    let result = (bash -c $run_cmd | complete)
    if $rule.id == "clippy" {
        if (which cargo | is-empty) { return [] }
        $result.stdout | parse-clippy-output
    } else {
        if $result.exit_code != 0 { return [] }
        let severity = ($rule.severity? | default "warning")
        let message = ($rule.message? | default "")
        $result.stdout | parse-vimgrep-output $rule.id $severity $message
    }
}

# Check if path matches glob pattern like **/target/** by testing if path contains the literal parts
def matches-ignore [file: string, pattern: string]: nothing -> bool {
    let parts = ($pattern | split row "**" | where { $in | is-not-empty })
    if ($parts | is-empty) { return true }

    $parts | all {|part|
        let cleaned = ($part | str trim --char "/")
        ($cleaned | is-empty) or ($file | str contains $cleaned)
    }
}

# Run a file-metric custom rule (e.g., line-count checks)
def run-file-metric-rule [rule: record, project_root: string]: nothing -> list {
    let check = ($rule.check? | default "")
    if $check != "line-count" { return [] }

    let max_lines = ($rule.max? | default 0)
    let severity = ($rule.severity? | default "warning")
    let message = ($rule.message? | default "")
    let file_patterns = ($rule.files? | default [])
    let ignore_patterns = ($rule.ignores? | default [])

    cd $project_root

    $file_patterns
    | each {|pattern|
        glob $pattern
    }
    | flatten
    | where {|file|
        ($file | path type) == "file"
    }
    | where {|file|
        # Check if file matches any ignore pattern
        let file_str = ($file | into string)
        not ($ignore_patterns | any {|ignore|
            matches-ignore $file_str $ignore
        })
    }
    | each {|file|
        let lines = (open $file | lines | length)
        if $lines > $max_lines {
            make-finding $rule.id $severity $message ($file | path relative-to $project_root | default $file) 0 0 0 0 $"($lines) lines"
        } else {
            null
        }
    }
    | compact
}

# Run all custom rules (from LINTFRA_CUSTOM_RULES env as JSON)
def run-custom-rules [project_root: string]: nothing -> list {
    let rules_json = ($env.LINTFRA_CUSTOM_RULES? | default "[]")
    let rules = ($rules_json | from json)

    $rules
    | each {|rule|
        let rule_type = ($rule.type? | default "")
        match $rule_type {
            "command" => { run-command-rule $rule $project_root }
            "file-metric" => { run-file-metric-rule $rule $project_root }
            _ => { [] }
        }
    }
    | flatten
}

def format-findings-table []: list -> table {
    $in
    | each {|f|
        let line = ($f.range.start.line + 1)
        let sev = if $f.severity == "warning" { "warn" } else { $f.severity }
        let rule = $f.ruleId | str replace "clippy::" ""
        {
            sev: $sev
            rule: $rule
            info: $f.text
            location: $"($f.file):($line)"
        }
    }
    | uniq-by location rule
    | sort-by rule location
}

def format-summary-table []: list -> table {
    $in
    | group-by ruleId
    | transpose rule findings
    | each {|g|
        let first = ($g.findings | first)
        let sev = $first.severity
        {
            rule: ($g.rule | str replace "clippy::" "")
            count: ($g.findings | length)
            sev: (if $sev == "warning" { "warn" } else { $sev })
            message: $first.message
        }
    }
    | sort-by count --reverse
}

def main [
    --findings (-f)  # Output only findings table (for piping)
    --summary (-s)   # Output only summary table (for piping)
] {
    let project_root = (pwd)

    let ast_grep_findings = (run-ast-grep)
    let custom_findings = (run-custom-rules $project_root)
    let all_findings = ($ast_grep_findings | append $custom_findings)

    if ($all_findings | is-empty) {
        if not ($findings or $summary) { print "ok" }
        return
    }

    if $findings {
        $all_findings | format-findings-table
    } else if $summary {
        $all_findings | format-summary-table
    } else {
        print $"(ansi green)Findings(ansi reset)(ansi white):(ansi reset)"
        $all_findings | format-findings-table | print
        print $"\n(ansi green)Summary(ansi reset)(ansi white):(ansi reset)"
        $all_findings | format-summary-table | print
        exit 1
    }
}
