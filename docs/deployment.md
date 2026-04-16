# Deployment Guide

This guide covers deploying the Cilium IPIP Router in various environments.

## Prerequisites

- Kubernetes cluster (v1.34+)
- kubectl configured with cluster admin access
- Docker or containerd runtime
- Network overlay: Cilium CNI v1.18+

## Deployment Methods

### Method 1: Direct Container Deployment

```bash
# Build Docker image
docker build -t cilium-ipip-router:latest .

# Run in container
docker run -d \
  --name cilium-router \
  --network host \
  -v /etc/kubernetes/admin.conf:/root/.kube/config:ro \
  cilium-ipip-router:latest
```

### Method 2: Kubernetes Deployment

Create `deployment.yaml`:

```yaml
apiVersion: apps/v1
kind: Deployment
metadata:
  name: cilium-ipip-router
  namespace: kube-system
spec:
  replicas: 1
  selector:
    matchLabels:
      app: cilium-ipip-router
  template:
    metadata:
      labels:
        app: cilium-ipip-router
    spec:
      serviceAccountName: cilium-ipip-router
      containers:
      - name: router
        image: ghcr.io/blackcowmoo/cilium-ipip-router:latest
        imagePullPolicy: Always
        securityContext:
          runAsNonRoot: true
        env:
        - name: RUST_LOG
          value: info
        ports:
        - containerPort: 9090
          name: http
        resources:
          requests:
            memory: "64Mi"
            cpu: "100m"
          limits:
            memory: "128Mi"
            cpu: "200m"
```

### Method 3: Kubernetes Operator Pattern

For production use, consider deploying as a Kubernetes operator with:

- Custom Resource Definitions (CRDs)
- Operator SDK or kubebuilder
- RBAC configurations

## Kubernetes RBAC

Create `rbac.yaml`:

```yaml
apiVersion: v1
kind: ServiceAccount
metadata:
  name: cilium-ipip-router
  namespace: kube-system

---

apiVersion: rbac.authorization.k8s.io/v1
kind: ClusterRole
metadata:
  name: cilium-ipip-router
rules:
- apiGroups: [""]
  resources: ["nodes"]
  verbs: ["get", "watch", "list"]

---

apiVersion: rbac.authorization.k8s.io/v1
kind: ClusterRoleBinding
metadata:
  name: cilium-ipip-router
roleRef:
  apiGroup: rbac.authorization.k8s.io
  kind: ClusterRole
  name: cilium-ipip-router
subjects:
- kind: ServiceAccount
  name: cilium-ipip-router
  namespace: kube-system
```

Apply RBAC:

```bash
kubectl apply -f rbac.yaml
```

## Configuration

### Environment Variables

| Variable | Description | Default |
|----------|-------------|---------|
| `RUST_LOG` | Log level (trace, debug, info, warn, error) | `info` |
| `LISTEN_ADDR` | HTTP server address | `0.0.0.0:9090` |

### ConfigMap for Logging

```yaml
apiVersion: v1
kind: ConfigMap
metadata:
  name: cilium-ipip-router-config
  namespace: kube-system
data:
  log4rs.yaml: |
    refresh_rate: 30 seconds
    appenders:
      stdout:
        kind: console
    root:
      level: info
      appenders:
        - stdout
```

## Monitoring

### Health Checks

```bash
# Check endpoint
curl http://localhost:9090/health

# Expected response
"healthy"
```

### Metrics

The application exposes Prometheus metrics. Configure scrape jobs:

```yaml
scrape_configs:
  - job_name: 'cilium-ipip-router'
    static_configs:
      - targets: ['cilium-ipip-router:9090']
```

## Scaling

### Horizontal Scaling

- Deploy multiple replicas for high availability
- Use pod anti-affinity to distribute across nodes
- Configure readiness/liveness probes

```yaml
affinity:
  podAntiAffinity:
    preferredDuringSchedulingIgnoredDuringExecution:
    - weight: 100
      podAffinityTerm:
        labelSelector:
          matchExpressions:
          - key: app
            operator: In
            values:
            - cilium-ipip-router
        topologyKey: kubernetes.io/hostname
```

### Resource Tuning

Adjust based on cluster size:

```yaml
resources:
  requests:
    memory: "64Mi"
    cpu: "100m"
  limits:
    memory: "256Mi"
    cpu: "500m"
```

## Troubleshooting

### Logs

```bash
# View pod logs
kubectl logs -n kube-system -l app=cilium-ipip-router

# Follow logs
kubectl logs -f -n kube-system -l app=cilium-ipip-router
```

### Common Issues

**Connection Refused**
- Verify service account permissions
- Check network policies
- Ensure API server is accessible

**Watch Stream Errors**
- Verify RBAC for node watch permissions
- Check for network connectivity to API server
- Review log level for detailed errors

**Health Check Failures**
- Confirm server is running on port 9090
- Check for port conflicts
- Verify no firewall rules blocking traffic

## Maintenance

### Updates

```bash
# Update deployment
kubectl rollout update deployment/cilium-ipip-router -n kube-system

# Monitor rollout
kubectl rollout status deployment/cilium-ipip-router -n kube-system
```

### Backup

No persistent state required. Controller rebuilds state from Kubernetes API on restart.

### Rollback

```bash
# Rollback to previous version
kubectl rollout undo deployment/cilium-ipip-router -n kube-system
```
