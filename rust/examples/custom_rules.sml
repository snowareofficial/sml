@version v4

rules: [
  { match: "h1"      template: "# {value}\n" }
  { match: "h2"      template: "## {value}\n" }
  { match: "p"       template: "{value}\n\n" }
  { match: "item"    template: "- {value}\n" }
  { match: "*"       template: "{key}: {value}\n" }
]
