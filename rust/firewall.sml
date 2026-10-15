# firewall.sml — contracts, fragment reuse, env inlining
@contract Rule {
    action:  enum [ allow deny ]
    proto:   enum [ tcp udp icmp ]
    port:    int min 1 max 65535 optional
    source:  str
    log:     bool optional
}

@base { log: true }

Rule ssh {
    &base
    action: allow  proto: tcp  port: 22   source: 10.0.0.0/8
}

Rule web {
    &base
    action: allow  proto: tcp  port: 443  source: 0.0.0.0/0
}

Rule drop {
    @is Rule
    action: deny   proto: udp             source: 0.0.0.0/0
}

alert_webhook: $env.SLACK_WEBHOOK
