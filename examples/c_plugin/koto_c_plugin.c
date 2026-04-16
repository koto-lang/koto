#include "koto.h"

#include <stdint.h>
#include <stdlib.h>
#include <string.h>

static KStringSlice slice_from_cstr(const char *s) {
  KStringSlice slice;
  slice.ptr = (const unsigned char *)s;
  slice.len = strlen(s);
  return slice;
}

static KotoStatus ok_status(void) {
  KotoStatus status;
  status.code = KOTO_STATUS_CODE_OK;
  status.error = NULL;
  status.clone_error = NULL;
  status.free_error = NULL;
  status.is_unimplemented = false;
  status.message = NULL;
  return status;
}

static KotoStatus error_status(const char *message) {
  KotoStatus status = ok_status();
  status.code = KOTO_STATUS_CODE_ERROR;
  status.message = strdup(message);
  return status;
}

static KotoStatus sum_function(const struct KotoHostApiV1 *host_api,
                               CallContext ctx,
                               void *user_data,
                               KValue *out) {
  (void)host_api;
  (void)user_data;

  if (ctx.arg_count != 2) {
    return error_status("expected exactly 2 arguments");
  }

  const KValue *a = &ctx.args[0];
  const KValue *b = &ctx.args[1];
  if (a->kind != K_VALUE_KIND_I64 || b->kind != K_VALUE_KIND_I64) {
    return error_status("expected 2 i64 arguments");
  }

  *out = host_api->value_make_i64(a->data.i64_value + b->data.i64_value);
  return ok_status();
}

static void drop_noop(void *user_data) {
  (void)user_data;
}

KotoStatus koto_plugin_init_v1(const struct KotoHostApiV1 *host_api, KValue *out) {
  KMap exports = host_api->map_new_with_type(slice_from_cstr("c_example"));

  KValue answer = host_api->value_make_i64(42);
  host_api->map_insert_value(exports, slice_from_cstr("answer"), answer);

  KValue sum = {
      .kind = K_VALUE_KIND_NATIVE_FUNCTION,
      .data.native_function_value =
          host_api->native_function_make((KotoPluginFunction)sum_function, NULL, drop_noop),
  };
  host_api->map_insert_value(exports, slice_from_cstr("sum"), sum);

  out->kind = K_VALUE_KIND_MAP;
  out->data.map_value = exports;
  return ok_status();
}
