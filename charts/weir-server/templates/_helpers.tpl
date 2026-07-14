{{- define "weir-server.fullname" -}}
{{- printf "%s-weir-server" .Release.Name | trunc 63 | trimSuffix "-" -}}
{{- end -}}

{{- define "weir-server.labels" -}}
app.kubernetes.io/name: weir-server
app.kubernetes.io/instance: {{ .Release.Name }}
app.kubernetes.io/managed-by: {{ .Release.Service }}
{{- end -}}

{{- define "weir-server.selectorLabels" -}}
app.kubernetes.io/name: weir-server
app.kubernetes.io/instance: {{ .Release.Name }}
{{- end -}}

{{- define "weir-server.serviceAccountName" -}}
{{- if .Values.serviceAccount.create -}}
{{- default (include "weir-server.fullname" .) .Values.serviceAccount.name -}}
{{- else -}}
{{- default "default" .Values.serviceAccount.name -}}
{{- end -}}
{{- end -}}
