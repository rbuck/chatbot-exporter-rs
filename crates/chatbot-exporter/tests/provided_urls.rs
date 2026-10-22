use chatbot_exporter::{
    ChatGptProvider, ConversationProvider, default_output_name, parse_source_url,
};

struct Example {
    url: &'static str,
    title: &'static str,
    filename: &'static str,
}

const EXAMPLES: [Example; 18] = [
    Example {
        url: "https://chatgpt.com/share/6ad7b612-a6c0-83ea-8615-011cc0b67a2b",
        title: "qemu linux vm macos m4",
        filename: "qemu-linux-vm-macos-m4.md",
    },
    Example {
        url: "https://chatgpt.com/share/6ad7b623-2ee4-83e9-853a-3b46e2c3934e",
        title: "qemu vs vmware iac",
        filename: "qemu-vs-vmware-iac.md",
    },
    Example {
        url: "https://chatgpt.com/share/6ad7b634-78f0-83ea-a0d8-ced6e5f3c2d9",
        title: "create nfs shares debian",
        filename: "create-nfs-shares-debian.md",
    },
    Example {
        url: "https://chatgpt.com/share/6ad7b64a-4104-83ea-90d1-f758d7cea717",
        title: "debian qemu windows server 2016",
        filename: "debian-qemu-windows-server-2016.md",
    },
    Example {
        url: "https://chatgpt.com/share/6ad7b65b-90a0-83e9-926c-14c06c0e54be",
        title: "debian monitoring dashboard setup",
        filename: "debian-monitoring-dashboard-setup.md",
    },
    Example {
        url: "https://chatgpt.com/share/6ad7b66c-382c-83ea-aad5-a55abc9cd785",
        title: "suricata ruleset configuration",
        filename: "suricata-ruleset-configuration.md",
    },
    Example {
        url: "https://chatgpt.com/share/6ad7b67c-83f0-83e9-9b65-16e594165b6c",
        title: "debian router setup",
        filename: "debian-router-setup.md",
    },
    Example {
        url: "https://chatgpt.com/share/6ad7b68f-370c-83e9-9e95-75dc87c1f9d7",
        title: "core technologies in ipfire",
        filename: "core-technologies-in-ipfire.md",
    },
    Example {
        url: "https://chatgpt.com/share/6ad7b6aa-e578-83e9-b0cc-794b59bf84e1",
        title: "unbound replaces dnsmasq",
        filename: "unbound-replaces-dnsmasq.md",
    },
    Example {
        url: "https://chatgpt.com/share/6ad7b6bc-b4e4-83ea-a333-0385f1e5b0bc",
        title: "chroot for minimal router",
        filename: "chroot-for-minimal-router.md",
    },
    Example {
        url: "https://chatgpt.com/share/6ad7b6cd-4d80-83e9-811d-5225a15cdb5d",
        title: "linux led port flashing",
        filename: "linux-led-port-flashing.md",
    },
    Example {
        url: "https://chatgpt.com/share/6ad7b6e2-e2d8-83ea-aa93-ee5b8ad6ecf1",
        title: "alpine pxe boot setup",
        filename: "alpine-pxe-boot-setup.md",
    },
    Example {
        url: "https://chatgpt.com/share/6ad7b6f4-3394-83e9-9e65-989ceb483576",
        title: "apparmor en debian",
        filename: "apparmor-en-debian.md",
    },
    Example {
        url: "https://chatgpt.com/share/6ad7b708-7f4c-83ea-811e-2c55871ca631",
        title: "replacing router with debian",
        filename: "replacing-router-with-debian.md",
    },
    Example {
        url: "https://chatgpt.com/share/6ad7b71a-9a3c-83ea-86e7-7cf10168d890",
        title: "netboot linux automation",
        filename: "netboot-linux-automation.md",
    },
    Example {
        url: "https://chatgpt.com/share/6ad7b72b-1c54-83ea-b298-89428d5f2826",
        title: "minimal debian router setup",
        filename: "minimal-debian-router-setup.md",
    },
    Example {
        url: "https://chatgpt.com/share/6ad7b73d-e92c-83ea-bdea-bfae1f9f5d64",
        title: "lightweight linux routers",
        filename: "lightweight-linux-routers.md",
    },
    Example {
        url: "https://chatgpt.com/share/6ad7b750-b574-83e9-8427-bc56805b22d2",
        title: "debian router firewall setup",
        filename: "debian-router-firewall-setup.md",
    },
];

#[test]
fn validates_all_supplied_chatgpt_urls_and_expected_output_names() {
    for example in EXAMPLES {
        let url = parse_source_url(example.url).unwrap();
        assert!(
            ChatGptProvider.validate_url(&url).is_ok(),
            "{}",
            example.url
        );
        assert_eq!(
            default_output_name(Some(example.title)),
            example.filename,
            "{}",
            example.url
        );
    }
}
