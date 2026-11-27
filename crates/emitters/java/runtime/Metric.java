package dev.veld.runtime;

import java.util.Map;

/**
 * One measurement of the service-level hook (spec/platform.md section 13), with the names of the
 * OpenTelemetry HTTP semantic conventions so that an exporter needs no mapping; the generated code
 * depends on none.
 *
 * @param name {@code http.server.request.duration}
 * @param unit {@code s}
 * @param value the duration of the request in seconds
 * @param attributes {@code http.request.method}, {@code http.route} (the full path template),
 *     {@code http.response.status_code} (an {@code Integer}), {@code error.type} (the status as
 *     text, only for a {@code 5xx}), {@code veld.service} ({@code <package>/<Service>}), {@code
 *     veld.endpoint} and {@code veld.operation} (the whole operation signature of the endpoint)
 */
public record Metric(String name, String unit, double value, Map<String, Object> attributes) {}
