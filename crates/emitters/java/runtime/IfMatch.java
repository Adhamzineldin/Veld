package dev.veld.runtime;

/**
 * The {@code If-Match} of a call (spec/rules.md section 11.7): the version the caller read, or
 * "any". Build one with {@link #of(long)} or {@link #any()}. The client writes the header as the
 * server expects it and checks nothing: a malformed tag cannot be built from a number, and the
 * server answers {@code 422} for any header it does not understand.
 *
 * @param header the value of the header: {@code *} or the version as a strong entity tag
 */
public record IfMatch(String header) {

  /** The version the caller read; it is sent as the strong entity tag {@code "<version>"}. */
  public static IfMatch of(long version) {
    return new IfMatch("\"" + version + "\"");
  }

  /** Any current version: {@code *}. */
  public static IfMatch any() {
    return new IfMatch("*");
  }
}
