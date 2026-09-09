"""Deterministic regression tests for the real E2E shell assertions; no cluster."""
import os
from pathlib import Path
import subprocess
import unittest

SCRIPT = Path(__file__).resolve().parents[1] / "test_lifecycle.sh"
SETUP = r'''
source "$LIFECYCLE_SCRIPT"
trap - EXIT
synthetic_cidr=10.254.0.0/24
synthetic_ip=192.0.2.123
tunnel_name=tun-example
real_nodes="worker worker2"
get_router_pod_for_node() {
    [ "$1" != "${MISSING_NODE:-}" ] || return 0
    printf 'router-%s' "$1"
}
kubectl() {
    [[ "$*" != *"${FAIL_COMMAND:-NEVER_MATCH}"* ]] || return 1
    case "$*" in
        *'ip route show') printf '%s\n' "$ROUTES" ;;
        *'ip tunnel show') printf '%s\n' "$TUNNELS" ;;
        *'ip -o link show') printf '%s\n' "$LINKS" ;;
        *) return 99 ;;
    esac
}
'''


class LifecycleAssertions(unittest.TestCase):
    def run_shell(self, body, **values):
        env = dict(os.environ, LIFECYCLE_SCRIPT=str(SCRIPT))
        env.update(ROUTES="10.254.0.0/24 dev tun-example scope link",
                   TUNNELS="tun-example: ip/ip remote 192.0.2.123 local 172.18.0.2 ttl inherit",
                   LINKS="3: tun-example@NONE: <POINTOPOINT,NOARP,UP,LOWER_UP> mtu 1480")
        env.update(values)
        return subprocess.run(["bash", "-c", SETUP + body], env=env,
                              text=True, capture_output=True, timeout=5)

    def test_present_on_every_node(self):
        result = self.run_shell('state_matches_all_nodes present')
        self.assertEqual(result.returncode, 0, result.stderr)

    def test_absent_after_successful_reads(self):
        result = self.run_shell('state_matches_all_nodes absent', ROUTES="", TUNNELS="", LINKS="")
        self.assertEqual(result.returncode, 0, result.stderr)

    def test_exec_failures_never_prove_absence_or_presence(self):
        for command in ("ip route show", "ip tunnel show", "ip -o link show"):
            for state in ("present", "absent"):
                with self.subTest(command=command, state=state):
                    values = {} if state == "present" else dict(ROUTES="", TUNNELS="", LINKS="")
                    result = self.run_shell(f'state_matches_all_nodes {state}',
                                            FAIL_COMMAND=command, **values)
                    self.assertNotEqual(result.returncode, 0)

    def test_missing_router_cannot_be_skipped(self):
        for node in ("worker", "worker2"):
            for state in ("present", "absent"):
                with self.subTest(node=node, state=state):
                    values = {} if state == "present" else dict(ROUTES="", TUNNELS="", LINKS="")
                    result = self.run_shell(f'state_matches_all_nodes {state}', MISSING_NODE=node, **values)
                    self.assertNotEqual(result.returncode, 0)

    def test_empty_node_inventory_fails(self):
        self.assertNotEqual(self.run_shell('real_nodes=""; state_matches_all_nodes absent').returncode, 0)

    def test_incorrect_or_duplicate_state_fails(self):
        cases = [
            dict(ROUTES="10.254.0.0/24 dev tun-example-extra"),
            dict(ROUTES="10.254.0.0/25 dev tun-example"),
            dict(ROUTES="10.254.0.0/24 dev tun-example\n10.254.0.0/24 dev tun-example"),
            dict(TUNNELS="tun-example: ip/ip remote 192.0.2.1234 local 172.18.0.2"),
            dict(TUNNELS="tun-example: ip/ip remote 192.0.2.124 local 172.18.0.2"),
            dict(TUNNELS="tun-example: gre/ip remote 192.0.2.123 local 172.18.0.2"),
            dict(LINKS="3: tun-example@NONE: <POINTOPOINT,NOARP,LOWER_UP> mtu 1480"),
            dict(LINKS="3: tun-example-extra@NONE: <UP> mtu 1480"),
        ]
        for values in cases:
            with self.subTest(values=values):
                self.assertNotEqual(self.run_shell('state_matches_all_nodes present', **values).returncode, 0)

    def test_any_stale_resource_fails_absence(self):
        for resource, value in (("ROUTES", "10.254.0.0/24 dev wrong-device"),
                                ("TUNNELS", "tun-example: ip/ip remote 192.0.2.123"),
                                ("LINKS", "3: tun-example@NONE: <UP> mtu 1480")):
            with self.subTest(resource=resource):
                values = dict(ROUTES="", TUNNELS="", LINKS="")
                values[resource] = value
                self.assertNotEqual(self.run_shell('state_matches_all_nodes absent', **values).returncode, 0)

    def test_unrelated_resources_do_not_block_absence(self):
        result = self.run_shell('state_matches_all_nodes absent',
                                ROUTES="10.254.1.0/24 dev tun-other",
                                TUNNELS="tun-example-other: ip/ip remote 192.0.2.123",
                                LINKS="3: tun-example-other@NONE: <UP> mtu 1480")
        self.assertEqual(result.returncode, 0, result.stderr)

    def test_cleanup_preserves_failure_and_removes_owned_node(self):
        result = self.run_shell(r'''
synthetic_node=fixture-node
owns_node=true
collect_logs() { echo collected; return 1; }
kubectl() { echo "cleaned:$*"; }
assert_state_on_all_nodes() { echo "verified:$1"; }
trap cleanup_lifecycle EXIT
exit 7
''')
        self.assertEqual(result.returncode, 7)
        self.assertIn("collected", result.stdout)
        self.assertIn("verified:absent", result.stdout)

    def test_cleanup_failure_fails_successful_run(self):
        result = self.run_shell(r'''
synthetic_node=fixture-node
owns_node=true
kubectl() { return 1; }
assert_state_on_all_nodes() { return 0; }
trap cleanup_lifecycle EXIT
''')
        self.assertNotEqual(result.returncode, 0)

    def test_cleanup_does_not_delete_unowned_node(self):
        result = self.run_shell(r'''
owns_node=false
kubectl() { echo "unexpected-delete" >&2; return 1; }
trap cleanup_lifecycle EXIT
''')
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertNotIn("unexpected-delete", result.stderr)

    def test_full_lifecycle_recreates_with_new_endpoint_and_cleans_up(self):
        result = self.run_shell(r'''
cat() {
    if [ "${1:-}" = /proc/sys/kernel/random/uuid ]; then
        echo 00000000-0000-0000-0000-000000000001
    else
        command cat "$@"
    fi
}
get_all_nodes() { echo 'worker worker2'; }
collect_logs() { :; }
sleep() { echo 'Unexpected polling in synchronous fixture' >&2; exit 98; }
created=0
deleted=0
exists=false
kubectl() {
    case "$*" in
        'get nodes '*) echo '10.244.0.0/24 10.244.1.0/24' ;;
        'create -f -')
            cat >/dev/null
            [ "$exists" = false ] || return 1
            exists=true
            created=$((created + 1)) ;;
        'patch node '*)
            if [ "$created" -eq 1 ]; then
                [ "$synthetic_ip" = 192.0.2.123 ] || return 1
            else
                [ "$synthetic_ip" = 192.0.2.124 ] || return 1
            fi
            endpoint=$synthetic_ip ;;
        'delete node '*) exists=false; deleted=$((deleted + 1)) ;;
        *'ip route show')
            if [ "$exists" = true ]; then echo "$synthetic_cidr dev $tunnel_name"; fi ;;
        *'ip tunnel show')
            if [ "$exists" = true ]; then echo "$tunnel_name: ip/ip remote $endpoint"; fi ;;
        *'ip -o link show')
            if [ "$exists" = true ]; then echo "3: $tunnel_name@NONE: <UP> mtu 1480"; fi ;;
        *) return 99 ;;
    esac
}
main
[ "$created" -eq 2 ] && [ "$deleted" -eq 2 ] && [ "$owns_node" = false ]
''')
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)


if __name__ == "__main__":
    unittest.main()
