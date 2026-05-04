use crate::data::{Corpus, Record};
use crate::message::Message;
use std::collections::HashSet;
use std::fs;

pub fn execute() {
    let mut corpus = Corpus { items: vec![] };
    let mut seen = HashSet::new();

    let templates = vec![
        (vec!["ls", "list files", "show files", "what is here"], "ls"),
        (vec!["list all files", "show all", "list everything", "all files", "list all", "everything"], "ls -la"),
        (vec!["list hidden", "show hidden", "hidden files", "display hidden"], "ls -a"),
        (vec!["directory listing", "list directory", "dir", "list dir", "show directory"], "ls -l"),
        (vec!["list all directories", "show directories", "directories", "list folders"], "find . -type d"),
        (vec!["pwd", "where am i", "current directory", "print working folder", "working directory", "print dir"], "pwd"),
        (vec!["disk usage", "free disk", "how much disk left", "df", "disk space", "check disk"], "df -h"),
        (vec!["memory usage", "ram info", "free -h", "available ram", "memory status", "show ram"], "free -h"),
        (vec!["time", "date", "system date", "time now", "current date", "what is the date", "show date"], "date"),
        (vec!["my ip", "ip address", "network address", "ip a", "show ip", "what is my ip"], "ip addr"),
        (vec!["network interfaces", "ifconfig", "interfaces", "show interfaces", "list interfaces"], "ip addr show"),
        (vec!["ping google", "check google connectivity", "reach google", "google ping", "test google"], "ping -c 1 google.com"),
        (vec!["ping 8.8.8.8", "check 8.8.8.8", "connect to 8.8.8.8", "test 8.8.8.8", "ping dns"], "ping -c 1 8.8.8.8"),
        (vec!["ping localhost", "localhost ping", "reach localhost", "test localhost", "ping self"], "ping -c 1 127.0.0.1"),
        (vec!["docker containers", "list docker containers", "docker ps", "show running containers", "running containers"], "docker ps"),
        (vec!["all docker containers", "docker ps -a", "list all containers", "show all docker"], "docker ps -a"),
        (vec!["docker images", "list docker images", "show docker images", "images docker"], "docker images"),
        (vec!["listening ports", "open ports", "ss -tuln", "network sockets", "show ports", "what is listening"], "ss -tuln"),
        (vec!["find rust files", "find *.rs", "locate rust source", "rust files find", "rust sources"], "find . -name '*.rs'"),
        (vec!["find main.rs", "locate main.rs", "where is main.rs", "main.rs location"], "find . -name main.rs"),
        (vec!["find config.json", "locate config.json", "config.json path", "where config.json"], "find . -name config.json"),
        (vec!["search todo", "find todo comments", "look for todo", "grep TODO", "todo list"], "grep -r TODO src/"),
        (vec!["search main function", "find main def", "locate fn main", "grep main", "find main"], "grep -r 'fn main' src/"),
        (vec!["cat readme", "view readme", "show readme", "display readme.md", "read readme"], "cat README.md"),
        (vec!["cat cargo.toml", "show cargo.toml", "view cargo.toml", "display cargo", "show cargo"], "cat Cargo.toml"),
        (vec!["system info", "uname -a", "kernel info", "show uname", "system details"], "uname -a"),
        (vec!["kernel version", "uname -r", "kernel release", "release version", "kernel release"], "uname -r"),
        (vec!["hostname", "machine name", "computer name", "show hostname", "system name"], "hostname"),
        (vec!["who is logged in", "logged users", "who command", "show users", "current users"], "who"),
        (vec!["uptime", "how long running", "system uptime", "uptime now", "how long up"], "uptime"),
        (vec!["tail syslog", "last lines syslog", "end of syslog", "syslog tail"], "tail -n 20 /var/log/syslog"),
        (vec!["tail app.log", "app.log last lines", "tail -n 10 app.log", "show app log end"], "tail -n 10 app.log"),
        (vec!["head readme", "first lines readme", "top of readme", "head README.md"], "head -n 5 README.md"),
        (vec!["word count readme", "count lines readme", "wc readme", "lines number readme"], "wc -l README.md"),
        (vec!["env variables", "show env", "print environment", "env list", "display env"], "env"),
        (vec!["echo path", "show path", "print path variable", "path", "display path"], "echo $PATH"),
        (vec!["home directory", "echo home", "show home", "print home", "home"], "echo $HOME"),
        (vec!["shell name", "echo shell", "current shell", "print shell", "which shell", "echo $SHELL"], "echo $SHELL"),
        (vec!["whoami", "current user", "who am i", "show user", "my username"], "whoami"),
        (vec!["list processes", "ps aux", "running processes", "show procs", "process list"], "ps aux"),
        (vec!["process tree", "pstree", "tree of processes", "show pstree", "process tree view"], "pstree"),
        (vec!["kernel messages", "dmesg tail", "boot log", "latest dmesg", "kernel log"], "dmesg | tail -20"),
        (vec!["cpu info", "lscpu", "processor info", "show cpu", "cpu details"], "lscpu"),
        (vec!["block devices", "lsblk", "list disks", "show drives", "disk list"], "lsblk"),
        (vec!["mounted filesystems", "mount", "show mounts", "mounts list", "filesystems"], "mount"),
        (vec!["firewall status", "ufw status", "check firewall", "ufw", "firewall"], "ufw status"),
        (vec!["docker info", "systemwide docker info", "docker system df", "docker status", "docker overview"], "docker info"),
        (vec!["docker version", "version docker", "docker --version", "docker ver", "docker release"], "docker --version"),
        (vec!["systemctl status docker", "docker service status", "docker is active?", "status docker"], "systemctl status docker"),
        (vec!["systemctl status sshd", "ssh service status", "sshd active?", "status sshd"], "systemctl status sshd"),
        (vec!["systemctl status nginx", "nginx service status", "nginx active?", "status nginx"], "systemctl status nginx"),
        (vec!["systemctl is-active docker", "docker active check", "is docker running"], "systemctl is-active docker"),
        (vec!["systemctl is-active nginx", "nginx active check", "is nginx running"], "systemctl is-active nginx"),
        (vec!["systemctl list-units", "list all services", "systemctl units", "services list"], "systemctl list-units --type=service"),
        (vec!["journalctl -xe", "system logs", "view system journal", "journalctl", "system log"], "journalctl -xe"),
        (vec!["last reboot", "last boot time", "who -b", "system boot time", "last boot"], "who -b"),
        (vec!["last shutdown", "last shutdown time", "last -x", "previous shutdown"], "last -x | grep shutdown"),
        (vec!["find large files", "files bigger 100M", "large files search", "find large"], "find / -type f -size +100M 2>/dev/null"),
        (vec!["du -sh *", "disk usage summary", "folder sizes", "du current", "directory sizes"], "du -sh *"),
        (vec!["free memory buffers", "free -m buffers", "meminfo", "detailed memory"], "cat /proc/meminfo"),
        (vec!["cpu temperature", "sensors", "temp", "hardware temperature", "system sensors"], "sensors"),
        (vec!["ip link show", "network links", "links", "show network links"], "ip link show"),
        (vec!["route -n", "routing table", "netstat -rn", "ip route", "show routes"], "ip route"),
        (vec!["arp -a", "arp table", "neighbor table", "show arp"], "ip neigh"),
        (vec!["iostat", "io statistics", "disk io", "show iostat"], "iostat -x"),
        (vec!["vmstat", "virtual memory stats", "system vmstat", "vmstat 1 5"], "vmstat 1 5"),
        (vec!["top -bn1", "snapshot top", "one iteration top", "top once"], "top -bn1"),
        (vec!["whois example.com", "domain whois", "whois lookup", "check domain info"], "whois example.com"),
        (vec!["dig google.com", "dns lookup google", "google.com dns", "dig dns"], "dig google.com +short"),
        (vec!["nslookup google.com", "dns google", "resolve google.com", "lookup google"], "nslookup google.com"),
        (vec!["curl httpbin.org", "fetch httpbin", "http request test", "curl example"], "curl -s httpbin.org/get"),
        (vec!["wget example.com", "download example.com", "fetch index", "get example"], "wget -q -O- example.com"),
        (vec!["traceroute google.com", "trace route google", "path to google", "traceroute"], "traceroute -n google.com"),
        (vec!["mtr google.com", "mtr report", "my traceroute", "network path"], "mtr --report google.com"),
        (vec!["netstat -i", "network interfaces stats", "ifstat", "interface stats"], "netstat -i"),
        (vec!["lsusb", "usb devices", "list usb", "show usb"], "lsusb"),
        (vec!["lspci", "pci devices", "list pci", "show pci"], "lspci"),
        (vec!["lshw -short", "hardware summary", "hw list", "lshw short"], "lshw -short"),
        (vec!["hwinfo --short", "hardware info", "hwinfo", "system hardware"], "hwinfo --short"),
        (vec!["dmidecode -t memory", "memory hardware details", "ram info dmi", "dmi memory"], "sudo dmidecode -t memory"),
        (vec!["passwd -S user", "user password status", "check password", "passwd status"], "passwd -S $USER"),
        (vec!["id", "user id", "groups", "id command", "show uid"], "id"),
        (vec!["groups", "list groups", "group membership", "show groups"], "groups"),
        (vec!["crontab -l", "list cron jobs", "cron list", "show crontab"], "crontab -l"),
        (vec!["ls -la ~/", "home directory listing", "home dir contents", "list home", "home files"], "ls -la ~/"),
        (vec!["stat README.md", "file stat readme", "metadata README", "show stat readme"], "stat README.md"),
        (vec!["file Cargo.toml", "file type cargo", "what is Cargo.toml", "identify file", "check file Cargo"], "file Cargo.toml"),
        (vec!["file README.md", "check file readme", "identify readme"], "file README.md"),
        (vec!["list dirs only", "only directories", "folders only"], "ls -d */"),
        (vec!["show path env", "print $PATH", "path variable"], "echo $PATH"),
        (vec!["check disk inodes", "inode usage", "df -i", "show inodes"], "df -i"),
        (vec!["kernel modules", "lsmod", "loaded modules", "show modules"], "lsmod"),
        (vec!["last login", "last user login", "lastlog", "recent logins"], "lastlog"),
        (vec!["current runlevel", "who -r", "runlevel"], "who -r"),
    ];

    for (phrases, command) in templates {
        for phrase in phrases {
            let record = Record {
                phrase: Message::Text(phrase.to_string()),
                command: Message::Command(command.to_string()),
            };
            let key = (format!("{}", record.phrase), format!("{}", record.command));
            if seen.insert(key) {
                corpus.items.push(record);
            }
        }
    }

    let content = serde_json::to_string_pretty(&corpus).unwrap();
    fs::write("dataset.json", content).unwrap();
    println!("generated dataset ({} items)", corpus.items.len());
}