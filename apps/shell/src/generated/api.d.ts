// Generated from iris-daemon OpenAPI. Do not edit.
// Contract SHA-256: 1e05fabc3ab0306b246f12bb1e3288a4736ada197a2c831a7e38cd98ae464414
export interface paths {
    "/api/v1/bootstrap": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["bootstrap"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/v1/devices/gpu": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["gpu_devices"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/v1/edit-presets": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["editing_list_presets"];
        put?: never;
        post: operations["editing_create_preset"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/v1/edit-presets/{id}": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["editing_get_preset"];
        put: operations["editing_update_preset"];
        post?: never;
        delete: operations["editing_delete_preset"];
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/v1/edit-render/{session}": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        post?: never;
        /**
         * End a session and release its sequence watermark. Reopening clients should
         *     use a new identifier so abandoned requests cannot collide with new work.
         */
        delete: operations["editing_preview_close"];
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/v1/edit-render/{session}/cancel": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        post: operations["editing_preview_cancel"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/v1/edit-resources/import/{kind}": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        post: operations["editing_import_resource"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/v1/edit-resources/{hash}": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["editing_resource_bytes"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/v1/editing/capabilities": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["editing_capabilities"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/v1/edits/{id}": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["editing_get_edit"];
        put: operations["editing_save_edit"];
        post?: never;
        delete: operations["editing_delete_edit"];
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/v1/edits/{id}/batch": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        post: operations["editing_batch_settings"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/v1/edits/{id}/duplicate": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        post: operations["editing_duplicate_edit"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/v1/edits/{id}/histogram": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        post: operations["editing_preview_histogram"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/v1/edits/{id}/history": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["editing_edit_history"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/v1/edits/{id}/lens-profile": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        /**
         * Preparation never writes an edit. The caller saves chosen parameters using
         *     the existing expected-revision edit command after reviewing this response.
         */
        post: operations["editing_calibration_lens_profile"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/v1/edits/{id}/preview": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        post: operations["editing_preview_preview"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/v1/edits/{id}/preview-draft": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        /**
         * Transient slider feedback: validates source binding and resources but writes
         *     no history. Persist the chosen document separately through SaveEdit.
         */
        post: operations["editing_preview_draft_preview"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/v1/edits/{id}/proof": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        post: operations["editing_preview_proof"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/v1/edits/{id}/raw-white-balance": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        /**
         * RAW sensor gains, not a rendered RGB filter. Neutral sampling uses the
         *     oriented/cropped decoded source coordinates before any edit geometry.
         */
        post: operations["editing_calibration_raw_white_balance"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/v1/edits/{id}/recipe": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["editing_export_recipe"];
        put: operations["editing_import_recipe"];
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/v1/edits/{id}/redo": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        post: operations["editing_redo_edit"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/v1/edits/{id}/region": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        post: operations["editing_preview_region"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/v1/edits/{id}/sample": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        post: operations["editing_preview_sample"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/v1/edits/{id}/sync": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        post: operations["editing_sync_settings"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/v1/edits/{id}/undo": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        post: operations["editing_undo_edit"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/v1/models": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["models"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/v1/models/install": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["status"];
        put?: never;
        post: operations["start"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/v1/models/install/cancel": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        post: operations["cancel_model_install"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/v1/models/optional": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["catalog"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/v1/photos/{id}": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["photo"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/v1/photos/{id}/detail": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        post: operations["detail"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/v1/photos/{id}/edits": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["editing_list_edits"];
        put?: never;
        post: operations["editing_create_edit"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/v1/photos/{id}/metadata": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["photo_metadata"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/v1/photos/{id}/original": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["original"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/v1/photos/{id}/preview": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["preview"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/v1/photos/{id}/thumb": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["thumb"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/v1/profiles": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["profiles"];
        put?: never;
        post: operations["save_profile"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/v1/profiles/{name}": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        post?: never;
        delete: operations["delete_profile"];
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/v1/profiles/{name}/apply": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        post: operations["apply_profile"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/v1/projects": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["projects"];
        put?: never;
        post: operations["create_project"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/v1/projects/{id}": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["project"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/v1/projects/{id}/accept": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        post: operations["accept"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/v1/projects/{id}/analyze": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        post: operations["analyze"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/v1/projects/{id}/browse-context": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put: operations["browse_context"];
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/v1/projects/{id}/cache": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["cache_status"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/v1/projects/{id}/cache/cleanup": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        post: operations["cache_cleanup"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/v1/projects/{id}/cache/migrate": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        post: operations["cache_migrate"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/v1/projects/{id}/cache/migrations": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["cache_migrations"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/v1/projects/{id}/cache/migrations/{migration_id}/cleanup": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        post: operations["cache_cleanup_old"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/v1/projects/{id}/cancel": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        post: operations["cancel"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/v1/projects/{id}/compare": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        post: operations["compare_photos"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/v1/projects/{id}/decisions": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        post: operations["decisions"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/v1/projects/{id}/export/copy": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        post: operations["export_copy"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/v1/projects/{id}/export/csv": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        post: operations["export_csv"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/v1/projects/{id}/export/xmp": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        post: operations["export_xmp"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/v1/projects/{id}/groups": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["groups"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/v1/projects/{id}/groups/{group_id}/selection": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["group_selection"];
        put: operations["set_group_intent"];
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/v1/projects/{id}/groups/{group_id}/selection/accept": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        post: operations["accept_group_selection"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/v1/projects/{id}/hide": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        post: operations["hide_project"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/v1/projects/{id}/import/csv": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        post: operations["import_csv"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/v1/projects/{id}/marks": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        post: operations["marks"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/v1/projects/{id}/open": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        post: operations["open_project"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/v1/projects/{id}/pause": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        post: operations["pause"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/v1/projects/{id}/photos": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["photos"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/v1/projects/{id}/profiles/{name}/estimate": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        post: operations["estimate_profile"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/v1/projects/{id}/progress": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["progress"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/v1/projects/{id}/quarantine": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["quarantine_history"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/v1/projects/{id}/quarantine/commit": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        post: operations["quarantine_commit"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/v1/projects/{id}/quarantine/preview": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        post: operations["quarantine_preview"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/v1/projects/{id}/quarantine/restore": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        post: operations["quarantine_restore"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/v1/projects/{id}/resume": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        post: operations["resume"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/v1/projects/{id}/scan": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        post: operations["scan"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/v1/projects/{id}/scenes": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["scenes"];
        put: operations["correct_scene"];
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/v1/projects/{id}/search": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        post: operations["search"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/v1/projects/{id}/search/index": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        post: operations["index_search"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/v1/projects/{id}/search/status": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["search_status"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/v1/projects/{id}/undo": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        post: operations["undo"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/v1/rendered-exports": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["editing_exports_list"];
        put?: never;
        post: operations["editing_exports_queue"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/v1/rendered-exports/{id}": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["editing_exports_status"];
        put?: never;
        post?: never;
        delete: operations["editing_exports_delete"];
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/v1/rendered-exports/{id}/cancel": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        post: operations["editing_exports_cancel"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/v1/rendered-exports/{id}/recover": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        post: operations["editing_exports_recover"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/v1/rendered-exports/{id}/retry": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        post: operations["editing_exports_retry"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/v1/settings": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["settings"];
        put: operations["put_settings"];
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
}
export type webhooks = Record<string, never>;
export interface components {
    schemas: {
        /** @enum {string} */
        AcceptCategory: "all" | "recommend" | "reject_suggest";
        AcceptRequest: {
            /** @default all */
            category: components["schemas"]["AcceptCategory"];
            /**
             * @description Omitted means all active project photos; an empty list means no photos.
             * @default null
             */
            photo_ids: number[] | null;
        };
        /** @enum {string} */
        Action: "keep" | "reject" | "pending" | "flag";
        Adjustments: {
            /**
             * @default {
             *       "enabled": false,
             *       "mix": [
             *         0.26269999146461487,
             *         0.6779999732971191,
             *         0.059300001710653305
             *       ]
             *     }
             */
            black_and_white: components["schemas"]["BlackAndWhite"];
            /**
             * Format: float
             * @default 0
             */
            blacks: number;
            /**
             * @default {
             *       "points": []
             *     }
             */
            blue_curve: components["schemas"]["Curve"];
            /**
             * @default {
             *       "highlights": [
             *         0,
             *         0,
             *         0
             *       ],
             *       "midtones": [
             *         0,
             *         0,
             *         0
             *       ],
             *       "shadows": [
             *         0,
             *         0,
             *         0
             *       ]
             *     }
             */
            color_balance: components["schemas"]["ColorBalance"];
            /**
             * Format: float
             * @description Contrast exponent multiplier 2^contrast around pivot; signed power keeps negatives.
             * @default 0
             */
            contrast: number;
            /**
             * Format: float
             * @default 0.18000000715255737
             */
            contrast_pivot: number;
            /**
             * Format: float
             * @default 0
             */
            exposure: number;
            /**
             * @default {
             *       "amount": 0,
             *       "seed": 0
             *     }
             */
            grain: components["schemas"]["Grain"];
            /**
             * @default {
             *       "points": []
             *     }
             */
            green_curve: components["schemas"]["Curve"];
            /**
             * Format: float
             * @description Smooth masked log2 gains [-2,2], with separate broad/endpoint masks.
             * @default 0
             */
            highlights: number;
            /** @default [] */
            hsl: components["schemas"]["HueBand"][];
            /**
             * @default {
             *       "points": []
             *     }
             */
            master_curve: components["schemas"]["Curve"];
            /**
             * @default {
             *       "points": []
             *     }
             */
            red_curve: components["schemas"]["Curve"];
            /**
             * Format: float
             * @default 0
             */
            saturation: number;
            /**
             * Format: float
             * @default 0
             */
            shadows: number;
            /**
             * Format: float
             * @description Creative RGB gains in stops [-1,1]; not Kelvin or RAW illuminant correction.
             * @default 0
             */
            temperature: number;
            /**
             * Format: float
             * @default 0
             */
            tint: number;
            /**
             * @default {
             *       "darks": 0,
             *       "highlights": 0,
             *       "lights": 0,
             *       "shadows": 0
             *     }
             */
            tone_curve: components["schemas"]["ToneCurve"];
            /**
             * Format: float
             * @default 0
             */
            vibrance: number;
            /**
             * @default {
             *       "amount": 0,
             *       "feather": 1,
             *       "midpoint": 0.3499999940395355
             *     }
             */
            vignette: components["schemas"]["Vignette"];
            /**
             * Format: float
             * @default 0
             */
            whites: number;
        };
        AnalysisSettings: {
            /**
             * Format: int32
             * @default null
             */
            directml_device_id: number | null;
            /** @default null */
            embedding_model_sha256: string | null;
            /** @default none */
            embedding_provider: components["schemas"]["EmbeddingProvider"];
            /** @default true */
            enable_niqe: boolean;
            /** @default cpu */
            execution_provider: components["schemas"]["ExecutionProvider"];
            /**
             * Format: double
             * @default 20
             */
            exposure_weight: number;
            /**
             * Format: double
             * @default 25
             */
            eyes_weight: number;
            /**
             * Format: float
             * @default 0.550000011920929
             */
            face_confidence: number;
            /** @default yunet */
            face_detector: components["schemas"]["FaceDetectorProvider"];
            /**
             * Format: double
             * @default 20
             */
            face_weight: number;
            /** @default 10 */
            max_faces: number;
            /**
             * Format: double
             * @default 0.2
             */
            niqe_weight: number;
            /**
             * Format: float
             * @default null
             */
            occlusion_min_visible_fraction: number | null;
            /** @default null */
            occlusion_model_sha256: string | null;
            /** @default none */
            occlusion_provider: components["schemas"]["OcclusionProvider"];
            /**
             * Format: double
             * @default 70
             */
            recommend_threshold: number;
            /**
             * Format: double
             * @default 25
             */
            reject_threshold: number;
            /** @default null */
            scrfd_model_sha256: string | null;
            /**
             * Format: float
             * @default null
             */
            semantic_similarity_threshold: number | null;
            /**
             * Format: double
             * @default 25
             */
            sharpness_weight: number;
            /**
             * Format: double
             * @default 10
             */
            smile_weight: number;
        };
        /** @enum {string} */
        AnalysisStatus: "missing" | "current" | "stale";
        AnalyzeRequest: {
            /** @default false */
            retry_failed_only: boolean;
        };
        BatchEditTarget: {
            /** Format: int64 */
            expected_revision: number;
            id: string;
        };
        BatchSettings: {
            include_masks?: boolean;
            request_id: string;
            targets: components["schemas"]["BatchEditTarget"][];
        };
        BlackAndWhite: {
            /** @default false */
            enabled: boolean;
            /**
             * @description Linear RGB mix; coefficients in [-2,2], sum must be one.
             * @default [
             *       0.26269999146461487,
             *       0.6779999732971191,
             *       0.059300001710653305
             *     ]
             */
            mix: number[];
        };
        Bootstrap: {
            capabilities: string[];
            recovery_notice?: string | null;
            version: string;
        };
        /** @description Browsing changes only queued work, never starts a job or changes its scope. */
        BrowseContext: {
            /**
             * @description Stable id for one window; monotonically increasing revisions reject late requests.
             * @default
             */
            client_id: string;
            /** @default [] */
            comparison_photo_ids: number[];
            /**
             * Format: int64
             * @default null
             */
            focused_photo_id: number | null;
            /** @default [] */
            nearby_photo_ids: number[];
            /**
             * Format: int64
             * @default 0
             */
            revision: number;
        };
        BrushStroke: {
            /** Format: float */
            flow: number;
            /** Format: float */
            hardness: number;
            /** @description Polyline in normalized image coordinates, connected by continuous capsules. */
            points: number[][];
            /** Format: float */
            radius: number;
        };
        BurstGroup: {
            assessment?: components["schemas"]["GroupAssessment"] | null;
            id: string;
            kind: string;
            member_photo_ids: number[];
            /** Format: int64 */
            project_id: number;
        };
        CacheMigration: {
            created_at: string;
            destination_root: string;
            error?: string | null;
            files: components["schemas"]["CacheMigrationFile"][];
            id: string;
            /** Format: int64 */
            project_id: number;
            source_root: string;
            state: string;
        };
        CacheMigrationFile: {
            cleaned: boolean;
            path: string;
            sha256: string;
            /** Format: int64 */
            size_bytes: number;
        };
        CacheStatus: {
            /** Format: int64 */
            bytes: number;
            files: number;
            root: string;
        };
        CancelRequest: {
            /** Format: int64 */
            sequence: number;
        };
        ColorBalance: {
            /**
             * @default [
             *       0,
             *       0,
             *       0
             *     ]
             */
            highlights: number[];
            /**
             * @default [
             *       0,
             *       0,
             *       0
             *     ]
             */
            midtones: number[];
            /**
             * @description Per-channel log2 gains [-2,2], smoothly blended by linear Y.
             * @default [
             *       0,
             *       0,
             *       0
             *     ]
             */
            shadows: number[];
        };
        /** @enum {string} */
        ColorLabel: "none" | "red" | "yellow" | "green" | "blue" | "purple";
        ComparisonMember: {
            /** Format: double */
            exposure_mean_delta?: number | null;
            face_correspondences: components["schemas"]["FaceCorrespondence"][];
            face_count?: number | null;
            /** Format: int64 */
            photo_id: number;
            /** Format: double */
            score_delta?: number | null;
        };
        ComparisonReport: {
            assessment: components["schemas"]["GroupAssessment"];
            members: components["schemas"]["ComparisonMember"][];
            method: string;
            /** Format: int64 */
            reference_photo_id?: number | null;
        };
        ComparisonRequest: {
            photo_ids: number[];
            /** Format: int64 */
            reference_photo_id?: number | null;
        };
        /** @description Explainable portrait-placement geometry, not a learned aesthetic assessment. */
        Composition: {
            /** Format: double */
            center_distance: number;
            method: string;
            /** Format: double */
            score: number;
            subject: number[];
            /** Format: double */
            thirds_distance: number;
        };
        CreateProject: {
            auto_device?: boolean;
            root: string;
        };
        Curve: {
            /**
             * @description Empty means identity. Otherwise 2..=64 points, strictly increasing x,
             *     nondecreasing y, all coordinates in [0,1]. Endpoints must have x=0 and x=1.
             * @default []
             */
            points: components["schemas"]["CurvePoint"][];
        };
        CurvePoint: {
            /**
             * Format: float
             * @default 0
             */
            x: number;
            /**
             * Format: float
             * @default 0
             */
            y: number;
        };
        DecisionBatch: {
            changed: number;
            conflicts: number[];
            id: string;
        };
        DecisionRequest: {
            action: components["schemas"]["Action"];
            link_variants?: boolean;
            photo_ids: number[];
        };
        DestinationRequest: {
            destination: string;
        };
        DetailAdjustments: {
            /**
             * Format: float
             * @description Scalar atmospheric light in linear RGB [0.01,16].
             * @default 1
             */
            atmospheric_light: number;
            /**
             * Format: float
             * @default 0
             */
            chroma_denoise: number;
            /**
             * Format: float
             * @default 0
             */
            clarity: number;
            /**
             * Format: float
             * @description [-1,1]: negative adds veiling light; positive removes it.
             * @default 0
             */
            dehaze: number;
            /**
             * Format: float
             * @description Range sigma in linear-light units [0.001,1].
             * @default 0.03999999910593033
             */
            denoise_sigma: number;
            /**
             * Format: float
             * @description Blend with a joint bilateral estimate [0,1].
             * @default 0
             */
            luminance_denoise: number;
            /**
             * Format: float
             * @description Luminance unsharp gain [0,3].
             * @default 0
             */
            sharpening: number;
            /**
             * Format: float
             * @description Suppress differences smaller than this linear-light value [0,0.25].
             * @default 0
             */
            sharpening_threshold: number;
            /**
             * Format: float
             * @default 0
             */
            texture: number;
        };
        DetailEvidence: {
            /** Format: int32 */
            crop_height: number;
            /** Format: int32 */
            crop_width: number;
            /** Format: int32 */
            display_height?: number;
            /** Format: int32 */
            display_width?: number;
            exposure?: components["schemas"]["Exposure"] | null;
            /** @description Bounded JPEG of this original region; absent when native detail is unavailable. */
            image_data_url?: string | null;
            /** Format: int32 */
            measured_height: number;
            /** Format: int32 */
            measured_width: number;
            method: string;
            reason?: string | null;
            region: number[];
            /** Format: double */
            sharpness_lap?: number | null;
            /** Format: int32 */
            source_height: number;
            /** Format: int32 */
            source_width: number;
            /** @description full_detail, preview_only, or unavailable. No automatic reject decision. */
            state: string;
        };
        DetailRequest: {
            /** @default null */
            face_index: number | null;
            /**
             * @description x, y, width, height in the oriented image, normalized to 0..1.
             * @default null
             */
            region: number[] | null;
        };
        /** @description `available` means file/hash/metadata checks passed, not inference or accuracy validation. */
        DetectorModelStatus: {
            provider: components["schemas"]["FaceDetectorProvider"];
            reason?: string | null;
            /** @description Actual SHA-256 of bytes read; null when no model bytes could be inspected. */
            sha256?: string | null;
            state: components["schemas"]["ModelAvailability"];
        };
        DraftPreviewRequest: {
            document: components["schemas"]["EditDocument"];
            /** @description Revision is the saved base revision, not an unsaved counter. */
            render: components["schemas"]["RenderRequest"];
        };
        EditCommand: {
            /** Format: int64 */
            expected_revision: number;
            request_id: string;
        };
        EditDocument: {
            /** @description View-only data is excluded from the rendering fingerprint. */
            node_positions: {
                [key: string]: number[];
            };
            nodes: components["schemas"]["Node"][];
            output: components["schemas"]["OutputConfig"];
            output_node: string;
            panel_bindings: components["schemas"]["PanelBinding"][];
            /** Format: int32 */
            schema_version: number;
            source: components["schemas"]["SourceBinding"];
        };
        EditHistoryEntry: {
            action: string;
            created_at: string;
            /** Format: int64 */
            revision: number;
        };
        EditName: {
            name: string;
        };
        EditPreset: {
            created_at: string;
            id: string;
            name: string;
            recipe: components["schemas"]["EditRecipe"];
            updated_at: string;
        };
        /** @description Portable settings only: no photo identifier, filename or source fingerprint. */
        EditRecipe: {
            node_positions: {
                [key: string]: number[];
            };
            nodes: components["schemas"]["Node"][];
            output: components["schemas"]["OutputConfig"];
            output_node: string;
            panel_bindings: components["schemas"]["PanelBinding"][];
            raw_white_balance?: number[] | null;
            /** Format: int32 */
            schema_version: number;
        };
        EditRecord: {
            created_at: string;
            document: components["schemas"]["EditDocument"];
            id: string;
            name: string;
            /** Format: int64 */
            photo_id: number;
            /** Format: int64 */
            revision: number;
            updated_at: string;
        };
        EditResource: {
            hash: string;
            kind: components["schemas"]["EditResourceKind"];
            /** Format: int64 */
            size_bytes: number;
        };
        /** @enum {string} */
        EditResourceKind: "lut" | "icc" | "lens_profile";
        EditRevision: {
            /** Format: int64 */
            expected_revision: number;
        };
        EditSuccess: {
            ok: boolean;
        };
        /** @enum {string} */
        EditSyncGroup: "tone" | "color" | "curves" | "color_grading" | "detail" | "effects" | "crop" | "optics";
        EditingCapabilities: {
            compute_backend: string;
            /** Format: int32 */
            document_version: number;
            engine_version: string;
            limitations: string[];
            manual_lens_corrections: boolean;
            /** Format: int32 */
            max_preview_edge: number;
            /** Format: int64 */
            max_resource_bytes: number;
            preview_formats: string[];
            /** Format: int64 */
            render_memory_bytes: number;
            resource_kinds: string[];
            /** Format: int64 */
            source_cache_bytes: number;
            traditional_masks: boolean;
            working_precision: string;
            working_space: string;
        };
        EmbeddingModelStatus: {
            dimensions: number;
            preprocessing: string;
            provider: components["schemas"]["EmbeddingProvider"];
            reason?: string | null;
            /** @description SHA-256 verified against the pinned local artifact; absent on failure. */
            sha256?: string | null;
            state: components["schemas"]["ModelAvailability"];
        };
        /** @enum {string} */
        EmbeddingProvider: "none" | "dinov3_vits16";
        ExecutionFeedback: {
            completed_items: number;
            /** Format: int32 */
            device_id?: number | null;
            device_name?: string | null;
            phase: string;
            selected_provider: components["schemas"]["ExecutionProvider"];
            warnings: string[];
        };
        /** @enum {string} */
        ExecutionProvider: "cpu" | "directml" | "auto";
        ExportReport: {
            paths: string[];
            skipped: number;
            written: number;
        };
        ExportRequest: {
            destination?: string;
            overwrite?: boolean;
            scope?: string;
        };
        Exposure: {
            /** Format: double */
            highlight_clip: number;
            /** Format: double */
            mean: number;
            /** Format: double */
            shadow_clip: number;
            verdict: string;
        };
        Eye: {
            /** Format: float */
            blink_score?: number | null;
            /** Format: float */
            ear: number;
            state: components["schemas"]["EyeState"];
        };
        /** @enum {string} */
        EyeState: "open" | "closed" | "uncertain";
        /** @description Visible-face mask measurements in a fixed eye-corner ROI, not calibrated occlusion probabilities. */
        EyeVisibility: {
            /** Format: float */
            mean_probability: number;
            method: string;
            sampled_pixels: number;
            /** Format: float */
            visible_fraction: number;
        };
        Face: {
            bbox: number[];
            /** Format: float */
            confidence: number;
            head_pose?: components["schemas"]["HeadPose"] | null;
            index: number;
            /** Format: float */
            landmark_confidence?: number | null;
            landmarks: number[][];
            left_eye?: components["schemas"]["Eye"] | null;
            left_eye_unreliable_reason?: string | null;
            left_eye_visibility?: components["schemas"]["EyeVisibility"] | null;
            quality?: components["schemas"]["FaceQuality"] | null;
            quality_unavailable_reason?: string | null;
            right_eye?: components["schemas"]["Eye"] | null;
            right_eye_unreliable_reason?: string | null;
            right_eye_visibility?: components["schemas"]["EyeVisibility"] | null;
            /** Format: float */
            smile_score?: number | null;
            unreliable_reason?: string | null;
        };
        FaceChoice: {
            /** @description Echo the current key returned by group selection; stale corrections are rejected. */
            analysis_key: string;
            /** @description Null explicitly means that the main person is absent in this photo. */
            face_index?: number | null;
            /** Format: int64 */
            photo_id: number;
        };
        FaceCorrespondence: {
            face_index?: number | null;
            reference_face_index: number;
            /** @description geometry_candidate or uncertain; users must verify correspondence. */
            state: string;
        };
        /** @enum {string} */
        FaceDetectorProvider: "yunet" | "scrfd_500m";
        FaceQuality: {
            /** Format: int32 */
            crop_height: number;
            /** Format: int32 */
            crop_width: number;
            exposure: components["schemas"]["Exposure"];
            /** Format: double */
            exposure_score: number;
            method: string;
            /** Format: double */
            resolution_score: number;
            /** Format: double */
            score: number;
            /** Format: double */
            sharpness_lap: number;
            /** Format: double */
            sharpness_score: number;
        };
        Geometry: {
            /**
             * @description Additional red/blue radial scale relative to green, bounded to +/- 0.1.
             * @default [
             *       0,
             *       0
             *     ]
             */
            chromatic_aberration: number[];
            /**
             * @description [left, top, width, height], normalized to the input frame.
             * @default [
             *       0,
             *       0,
             *       1,
             *       1
             *     ]
             */
            crop: number[];
            /**
             * @description Row-major inverse homography in normalized image coordinates.
             * @default [
             *       1,
             *       0,
             *       0,
             *       0,
             *       1,
             *       0,
             *       0,
             *       0,
             *       1
             *     ]
             */
            perspective: number[];
            /**
             * @description Inverse radial mapping r -> r*(1+k1*r²+k2*r⁴), radius in half short edge.
             * @default [
             *       0,
             *       0
             *     ]
             */
            radial: number[];
            /**
             * Format: float
             * @description Clockwise displayed rotation/straightening, degrees.
             * @default 0
             */
            rotation_degrees: number;
            /**
             * Format: float
             * @description Exposure lift at radius 1 in stops (quadratic radial rolloff).
             * @default 0
             */
            vignette_stops: number;
        };
        GpuAdapter: {
            /** Format: int64 */
            dedicated_system_memory_bytes: number;
            /**
             * Format: int64
             * @description Adapter capacities, not current memory usage or free-memory budgets.
             */
            dedicated_video_memory_bytes: number;
            /**
             * Format: int32
             * @description Pass unchanged as AnalysisSettings.directml_device_id. Never reindex a filtered list.
             */
            device_id: number;
            /** Format: int32 */
            hardware_device_id: number;
            /** @description D3D12 UMA query; absent when this adapter cannot create a D3D12 device. */
            is_integrated?: boolean | null;
            is_remote: boolean;
            is_software: boolean;
            /** @description DXGI adapter LUID, useful within the current Windows boot; not a persistent ID. */
            luid: string;
            name: string;
            /** Format: int64 */
            shared_system_memory_bytes: number;
            /** Format: int32 */
            vendor_id: number;
        };
        GpuDevices: {
            adapters: components["schemas"]["GpuAdapter"][];
            /**
             * Format: int32
             * @description DirectML's default index 0 when it exists, not a fastest-device recommendation.
             */
            default_device_id?: number | null;
            /** @description Always false: presence does not establish runtime/model compatibility or speed. */
            inference_verified: boolean;
            reason?: string | null;
            /** @description "dxgi_enum_adapters" on Windows, "unsupported" elsewhere. */
            source: string;
            status: components["schemas"]["GpuEnumerationStatus"];
        };
        /** @enum {string} */
        GpuEnumerationStatus: "available" | "unavailable" | "unsupported";
        Grain: {
            /**
             * Format: float
             * @description Additive uniform noise amplitude in linear units [0,0.25].
             * @default 0
             */
            amount: number;
            /**
             * Format: int32
             * @default 0
             */
            seed: number;
        };
        GroupAssessment: {
            candidate_photo_ids: number[];
            /** Format: double */
            close_score_gap: number;
            comparisons: components["schemas"]["GroupComparison"][];
            method: string;
            needs_review: boolean;
            /**
             * Format: int64
             * @description Comparison reference only. Never an instruction to discard other members.
             */
            reference_photo_id?: number | null;
        };
        GroupComparison: {
            /** Format: int64 */
            photo_id: number;
            reasons: string[];
            /** Format: double */
            score_gap?: number | null;
        };
        GroupFace: {
            analysis_key?: string | null;
            face_index?: number | null;
            /** Format: int64 */
            photo_id: number;
            /** @description reference, manual, absent, geometry_candidate, uncertain, or stale. */
            state: string;
        };
        GroupIntent: {
            /** @default [] */
            face_choices: components["schemas"]["FaceChoice"][];
            /** @default [] */
            lock_baseline: number[];
            /** @default [] */
            locked_photo_ids: number[];
            /** @default null */
            main_face: components["schemas"]["FaceChoice"] | null;
            /** @default null */
            target_keep: number | null;
        };
        GroupSelection: {
            faces: components["schemas"]["GroupFace"][];
            group_id: string;
            intent: components["schemas"]["GroupIntent"];
            needs_review: boolean;
            over_target: number;
            pending_keep_photo_ids: number[];
            recommended_photo_ids: number[];
            remaining_slots: number;
        };
        HeadPose: {
            method: string;
            /** Format: float */
            pitch: number;
            /** Format: float */
            roll: number;
            /** Format: float */
            yaw: number;
        };
        Histogram: {
            above: number[];
            below: number[];
            /** @description RGB and luminance, excluding fully transparent pixels. */
            bins: number[][];
            domain: components["schemas"]["ScopeDomain"];
            /** Format: float */
            lower: number;
            /** Format: float */
            upper: number;
            /** Format: int64 */
            visible_pixels: number;
        };
        HistogramRequest: {
            /** Format: int32 */
            bins: number;
            domain: components["schemas"]["ScopeDomain"];
            /** Format: float */
            lower: number;
            render: components["schemas"]["RenderRequest"];
            /** Format: float */
            upper: number;
        };
        HistogramResponse: {
            approximate: boolean;
            fingerprint: string;
            histogram: components["schemas"]["Histogram"];
            /** Format: int32 */
            preview_height: number;
            /**
             * Format: int32
             * @description Full-quality requests measure every output pixel; interactive requests
             *     measure the displayed proxy. These are the actual measured dimensions.
             */
            preview_width: number;
            /** Format: int64 */
            revision: number;
            /** Format: int64 */
            sequence: number;
        };
        HueBand: {
            /**
             * Format: float
             * @description Centre in degrees [0,360]; wrap-around is supported.
             * @default 0
             */
            center: number;
            /**
             * Format: float
             * @default 0
             */
            hue_shift: number;
            /**
             * Format: float
             * @description Additive linear luminance change [-1,1], scaled by chroma selection.
             * @default 0
             */
            luminance: number;
            /**
             * Format: float
             * @description Relative chroma change [-1,1].
             * @default 0
             */
            saturation: number;
            /**
             * Format: float
             * @description Half-width in degrees (1..180), triangular selection weight.
             * @default 45
             */
            width: number;
        };
        ImportRecipe: {
            command: components["schemas"]["EditCommand"];
            recipe: components["schemas"]["EditRecipe"];
        };
        InstallProgress: {
            completed_bytes: number;
            error?: string | null;
            model: string;
            state: string;
            total_bytes: number;
        };
        InstallRequest: {
            model: string;
            /** @description Omitted selects the pinned public download; otherwise read this local folder. */
            source_folder?: string | null;
        };
        JobProgress: {
            completed: number;
            errors: string[];
            execution?: components["schemas"]["ExecutionFeedback"][];
            failed_photo_ids?: number[];
            failed_scan_paths?: string[];
            found_photos?: number;
            id?: string;
            kind: string;
            phase?: string;
            previous_execution?: components["schemas"]["ExecutionFeedback"][];
            /** @description Quality results are committed; optional semantic grouping may still be running. */
            quality_ready?: boolean;
            result?: unknown;
            root_unavailable?: boolean;
            state: string;
            total: number;
        };
        /** @enum {string} */
        LensCalibrationModel: "iris_inverse_radial_v1";
        LensCaptureMetadata: {
            camera_make?: string | null;
            camera_model?: string | null;
            dimensions: number[];
            /** Format: double */
            f_number?: number | null;
            /** Format: double */
            focal_mm?: number | null;
            lens_model?: string | null;
        };
        /** @enum {string} */
        LensCoordinates: "oriented_uncropped_image_edges_half_short_edge_v1";
        LensCorrection: {
            /** @description Additional red/blue scale relative to green, after the radial mapping. */
            chromatic_aberration: number[];
            /** @description Inverse r -> r*(1+k1*r^2+k2*r^4), radius in half the short image edge. */
            radial: number[];
            /**
             * Format: float
             * @description Exposure lift: 2^(stops * min(r^2,1)), evaluated at mapped input position.
             */
            vignette_stops: number;
        };
        LensProfile: {
            /** @description Exact developed source extent; transposed dimensions permit EXIF rotation. */
            calibrated_dimensions: number[];
            camera_make: string;
            camera_model: string;
            coordinates: components["schemas"]["LensCoordinates"];
            correction: components["schemas"]["LensCorrection"];
            f_number: number[];
            focal_mm: number[];
            lens_model: string;
            model: components["schemas"]["LensCalibrationModel"];
            name: string;
            /** Format: int32 */
            schema_version: number;
        };
        LensProfileMatch: {
            applicable: boolean;
            correction?: components["schemas"]["LensCorrection"] | null;
            /** @description Stable reason codes; no fuzzy matching or fallback to file names. */
            issues: string[];
        };
        LensProfileSuggestion: {
            capture: components["schemas"]["LensCaptureMetadata"];
            document_id: string;
            /** Format: int64 */
            expected_revision: number;
            matching: components["schemas"]["LensProfileMatch"];
            /**
             * @description Merge optical coefficients into a node using the uncropped source extent.
             *     This response never mutates a document or replaces crop/rotation/perspective.
             */
            placement: string;
            resource: string;
            source_fingerprint: string;
        };
        LensSuggestionRequest: {
            /** Format: int64 */
            expected_revision: number;
            resource: string;
        };
        /** @enum {string} */
        LutDomain: "linear_rec2020" | "encoded_srgb";
        ManifestRequest: {
            manifest_id: string;
        };
        MarkRequest: {
            color_label?: components["schemas"]["ColorLabel"] | null;
            decision?: components["schemas"]["Action"] | null;
            photo_ids: number[];
            /** Format: int32 */
            rating?: number | null;
        };
        MaskDefinition: {
            /** @enum {string} */
            kind: "brush";
            strokes: components["schemas"]["BrushStroke"][];
        } | {
            end: number[];
            /** @enum {string} */
            kind: "linear_gradient";
            start: number[];
        } | {
            center: number[];
            /** Format: float */
            hardness: number;
            /** @enum {string} */
            kind: "radial_gradient";
            radii: number[];
        } | {
            /** Format: float */
            feather: number;
            /** Format: float */
            high: number;
            /** @enum {string} */
            kind: "luminance_range";
            /** Format: float */
            low: number;
        } | {
            color: number[];
            /** Format: float */
            feather: number;
            /** @enum {string} */
            kind: "color_range";
            /** Format: float */
            radius: number;
        } | {
            /** @enum {string} */
            kind: "invert";
            mask: components["schemas"]["MaskDefinition"];
        } | {
            /** @enum {string} */
            kind: "combine";
            left: components["schemas"]["MaskDefinition"];
            operation: components["schemas"]["MaskOperation"];
            right: components["schemas"]["MaskDefinition"];
        } | {
            /** @enum {string} */
            kind: "opacity";
            mask: components["schemas"]["MaskDefinition"];
            /** Format: float */
            opacity: number;
        };
        /** @enum {string} */
        MaskOperation: "add" | "subtract" | "intersect";
        /** @enum {string} */
        ModelAvailability: "disabled" | "available" | "missing" | "invalid";
        ModelStatusResponse: {
            detectors: components["schemas"]["DetectorModelStatus"][];
            embedding_selected: components["schemas"]["EmbeddingProvider"];
            embeddings: components["schemas"]["EmbeddingModelStatus"][];
            occlusion: components["schemas"]["OcclusionModelStatus"][];
            occlusion_selected: components["schemas"]["OcclusionProvider"];
            selected: components["schemas"]["FaceDetectorProvider"];
        };
        Node: {
            enabled: boolean;
            id: string;
            inputs: string[];
            name: string;
            operation: components["schemas"]["Operation"];
            /** Format: int32 */
            version: number;
        };
        OcclusionModelStatus: {
            provider: components["schemas"]["OcclusionProvider"];
            reason?: string | null;
            /** @description Actual hash of read bytes. Available only describes artifact checks, not inference or licensing. */
            sha256?: string | null;
            state: components["schemas"]["ModelAvailability"];
        };
        /** @enum {string} */
        OcclusionProvider: "none" | "faceocc";
        Operation: {
            /** @enum {string} */
            type: "source";
        } | {
            parameters: components["schemas"]["Adjustments"];
            /** @enum {string} */
            type: "adjustments";
        } | {
            parameters: components["schemas"]["DetailAdjustments"];
            /** @enum {string} */
            type: "detail";
        } | {
            parameters: components["schemas"]["Geometry"];
            /** @enum {string} */
            type: "geometry";
        } | {
            parameters: components["schemas"]["MaskDefinition"];
            /** @enum {string} */
            type: "mask";
        } | {
            /** @description Inputs: base Image, adjusted Image, Mask. Linear-light interpolation. */
            parameters: {
                /** Format: float */
                opacity: number;
            };
            /** @enum {string} */
            type: "blend";
        } | {
            parameters: {
                domain: components["schemas"]["LutDomain"];
                resource: string;
                /** Format: float */
                strength: number;
            };
            /** @enum {string} */
            type: "lut";
        };
        OptionalModel: {
            bytes: number;
            id: string;
            license: string;
            license_url: string;
            sha256: string;
            state: string;
            title: string;
        };
        OutputConfig: {
            /**
             * @description Opaque JPEG background, specified as encoded sRGB values.
             * @default [
             *       1,
             *       1,
             *       1
             *     ]
             */
            background: number[];
            /**
             * @description Neutral-axis gamut compression in the target's linear RGB primaries.
             * @default true
             */
            gamut_compression: boolean;
            /** @default srgb */
            space: components["schemas"]["OutputSpace"];
            /**
             * @default {
             *       "kind": "none"
             *     }
             */
            tone_map: components["schemas"]["ToneMap"];
        };
        OutputFormat: {
            /** @enum {string} */
            format: "jpeg8";
            /** Format: int32 */
            quality: number;
        } | {
            /** @enum {string} */
            format: "png16";
        } | {
            /** @enum {string} */
            format: "tiff16";
        };
        /** @enum {string} */
        OutputSpace: "srgb" | "display_p3" | "adobe_rgb" | "linear_rec2020";
        PanelBinding: {
            node_id: string;
            role: string;
        };
        Photo: {
            analysis?: components["schemas"]["VisionAnalysis"] | null;
            /** @description Cache compatibility with the engine/settings and the last scanned source metadata. */
            analysis_status: components["schemas"]["AnalysisStatus"];
            capture_variant_id?: string | null;
            color_label?: components["schemas"]["ColorLabel"];
            decision: components["schemas"]["Action"];
            filename: string;
            format: string;
            /** Format: int32 */
            height: number;
            /** Format: int64 */
            id: number;
            missing: boolean;
            /** Format: int64 */
            mtime: number;
            path: string;
            /** Format: int64 */
            project_id: number;
            quarantined: boolean;
            /** Format: int32 */
            rating?: number;
            /** Format: int64 */
            size_bytes: number;
            taken_at?: string | null;
            /** Format: int32 */
            width: number;
        };
        PhotoFilter: {
            color_label?: components["schemas"]["ColorLabel"] | null;
            decision?: components["schemas"]["Action"] | null;
            descending?: boolean | null;
            format?: string | null;
            include_missing?: boolean | null;
            limit?: number | null;
            offset?: number | null;
            /**
             * Format: int64
             * @description Reveal a search hit without loading an entire project into the frontend.
             */
            photo_id?: number | null;
            /** Format: int32 */
            rating?: number | null;
            /** @description Transient scene navigation scope; other filters and pagination still apply. */
            scene_id?: string | null;
            sort?: string | null;
            verdict?: string | null;
        };
        PhotoMetadata: {
            artist?: string | null;
            camera_make?: string | null;
            camera_model?: string | null;
            captured_at?: string | null;
            captured_offset?: string | null;
            copyright?: string | null;
            /** Format: double */
            exposure_bias_ev?: number | null;
            /** Format: int32 */
            exposure_program?: number | null;
            /** Format: double */
            exposure_seconds?: number | null;
            /** Format: double */
            f_number?: number | null;
            /** Format: int32 */
            flash?: number | null;
            /** Format: int32 */
            focal_35mm?: number | null;
            /** Format: double */
            focal_mm?: number | null;
            /** Format: int32 */
            iso?: number | null;
            lens_make?: string | null;
            lens_model?: string | null;
            /** Format: int32 */
            metering_mode?: number | null;
            /** Format: int32 */
            orientation?: number | null;
            status: components["schemas"]["PhotoMetadataStatus"];
            /** Format: int32 */
            white_balance?: number | null;
        };
        /** @enum {string} */
        PhotoMetadataStatus: "available" | "not_found" | "unreadable" | "read_limit";
        PixelRect: {
            /** Format: int32 */
            height: number;
            /** Format: int32 */
            width: number;
            /** Format: int32 */
            x: number;
            /** Format: int32 */
            y: number;
        };
        PrepareWhiteBalanceRequest: {
            /** Format: int64 */
            expected_revision: number;
            request: components["schemas"]["RawWhiteBalanceRequest"];
        };
        PreparedRawWhiteBalance: {
            calibration: components["schemas"]["RawCalibration"];
            gains_rgb: number[];
            /** Format: int64 */
            revision: number;
            /** Format: int32 */
            sampled_pixels?: number | null;
            source_fingerprint: string;
            temperature?: components["schemas"]["WhiteBalanceTemperature"] | null;
        };
        PresetSettings: {
            name: string;
            recipe: components["schemas"]["EditRecipe"];
        };
        Profile: {
            name: string;
            settings: components["schemas"]["AnalysisSettings"];
        };
        Project: {
            cache_root: string;
            created_at: string;
            /** @description Group membership/ranking must be rebuilt, including after reopening a project. */
            groups_dirty: boolean;
            hidden: boolean;
            /** Format: int64 */
            id: number;
            last_opened_at: string;
            name: string;
            /**
             * Format: int64
             * @description Active photos without current-engine analysis, independent of pagination.
             */
            pending_analysis: number;
            root: string;
        };
        ProjectRequest: {
            /** Format: int64 */
            project_id: number;
        };
        ProofRequest: {
            document?: components["schemas"]["EditDocument"] | null;
            render: components["schemas"]["RenderRequest"];
            resource: string;
        };
        QuarantineItem: {
            destination: string;
            /** Format: int64 */
            photo_id: number;
            sha256: string;
            /** Format: int64 */
            size_bytes: number;
            source: string;
            state: string;
        };
        QuarantinePlan: {
            id: string;
            items: components["schemas"]["QuarantineItem"][];
            /** Format: int64 */
            project_id: number;
            state: string;
        };
        QueueExports: {
            exports: components["schemas"]["RenderedExportRequest"][];
        };
        RawCalibration: {
            applied_gains: number[];
            as_shot_gains?: number[] | null;
            /** @description Maps WB-multiplied camera RGB to linear Rec.2020; D65 neutral row normalized. */
            camera_to_working: number[][];
            /** Format: int32 */
            version: number;
            xyz_to_camera_d65: number[][];
        };
        RawWhiteBalanceRequest: {
            /** @enum {string} */
            kind: "as_shot";
        } | {
            gains_rgb: number[];
            /** @enum {string} */
            kind: "gains";
            locus: components["schemas"]["WhiteBalanceLocus"];
        } | {
            /** Format: double */
            kelvin: number;
            /** @enum {string} */
            kind: "temperature";
            locus: components["schemas"]["WhiteBalanceLocus"];
            /** Format: double */
            tint_delta_v: number;
        } | {
            /** @enum {string} */
            kind: "neutral";
            rect: components["schemas"]["PixelRect"];
        };
        /** @enum {string} */
        ReferenceKind: "scene" | "display";
        /**
         * @description Native 1:1 pixels in final edited-image coordinates. The entire graph is
         *     currently evaluated before cropping; max_edge does not resize this region.
         */
        RegionRequest: {
            document?: components["schemas"]["EditDocument"] | null;
            rect?: components["schemas"]["PixelRect"] | null;
            render: components["schemas"]["RenderRequest"];
            view?: components["schemas"]["RegionView"] | null;
        };
        /**
         * @description A native-pixel viewport centered in the final, edited image. Normalized
         *     coordinates avoid guessing dimensions after rotation, perspective or crop.
         */
        RegionView: {
            center: number[];
            /** Format: int32 */
            height: number;
            /** Format: int32 */
            width: number;
        };
        /** @enum {string} */
        RenderQuality: "interactive" | "full";
        RenderRequest: {
            /** Format: int32 */
            max_edge: number;
            quality: components["schemas"]["RenderQuality"];
            /** Format: int64 */
            revision: number;
            /** Format: int64 */
            sequence: number;
            session: string;
        };
        RenderedExportRequest: {
            destination_directory: string;
            document?: components["schemas"]["EditDocument"] | null;
            document_id: string;
            /** Format: int64 */
            expected_revision: number;
            filename: string;
            format: components["schemas"]["OutputFormat"];
        };
        /** @enum {string} */
        RenderedExportState: "queued" | "running" | "publishing" | "completed" | "failed" | "cancelled" | "interrupted";
        RenderedExportStatus: {
            batch_id: string;
            created_at: string;
            destination: string;
            document_id: string;
            error?: string | null;
            format: components["schemas"]["OutputFormat"];
            id: string;
            /** Format: int64 */
            result_bytes?: number | null;
            result_hash?: string | null;
            /** Format: int64 */
            revision: number;
            state: components["schemas"]["RenderedExportState"];
            updated_at: string;
        };
        RetrievalCoverage: {
            indexed: number;
            pending: number;
            total: number;
        };
        RetrievalStatus: {
            dimensions?: number | null;
            label?: string | null;
            reason?: string | null;
            space_key?: string | null;
            state: string;
        };
        SampleRequest: {
            rect: number[];
            render: components["schemas"]["RenderRequest"];
        };
        SampleResponse: {
            approximate: boolean;
            domain: string;
            fingerprint: string;
            /** Format: int32 */
            preview_height: number;
            /** Format: int32 */
            preview_width: number;
            /** Format: int64 */
            revision: number;
            rgb: number[];
            /** Format: int64 */
            sequence: number;
        };
        SaveEdit: {
            document: components["schemas"]["EditDocument"];
            /** Format: int64 */
            expected_revision: number;
            request_id: string;
        };
        ScanRequest: {
            /** @default false */
            retry_failed_only: boolean;
        };
        SceneCorrection: {
            photo_ids: number[];
            reset?: boolean;
        };
        SceneGroup: {
            burst_group_ids: string[];
            id: string;
            /** @description scene_candidate, manual, or unsequenced; never an exact-duplicate judgment. */
            kind: string;
            member_photo_ids: number[];
            reason: string;
        };
        /** @enum {string} */
        ScopeDomain: "linear_rec2020" | "encoded_srgb";
        ScoreBreakdown: {
            components: components["schemas"]["ScoreComponent"][];
            /** Format: double */
            effective_weight_total: number;
            method: string;
        };
        ScoreComponent: {
            /** Format: double */
            configured_weight: number;
            /** Format: double */
            contribution: number;
            /**
             * Format: double
             * @description Normalized share of the final score, in 0..1.
             */
            effective_weight: number;
            id: string;
            missing_reason?: string | null;
            observed_count: number;
            /** Format: double */
            score?: number | null;
            terms: components["schemas"]["ScoreTerm"][];
            total_count: number;
        };
        ScoreInput: {
            name: string;
            /** Format: double */
            value: number;
        };
        ScoreTerm: {
            /**
             * Format: double
             * @description Points inside the category, before its top-level weight.
             */
            contribution: number;
            id: string;
            mapping: string;
            missing_reason?: string | null;
            raw: components["schemas"]["ScoreInput"][];
            /** Format: double */
            score?: number | null;
            /**
             * Format: double
             * @description Actual normalized share inside this category; zero for missing terms.
             */
            weight: number;
        };
        SearchHit: {
            /** Format: int64 */
            photo_id: number;
            /** Format: double */
            similarity: number;
        };
        SearchRequest: {
            limit?: number | null;
            query: string;
        };
        SearchResponse: {
            coverage: components["schemas"]["RetrievalCoverage"];
            hits: components["schemas"]["SearchHit"][];
            space_key: string;
        };
        SearchStatus: {
            coverage?: components["schemas"]["RetrievalCoverage"] | null;
            model: components["schemas"]["RetrievalStatus"];
        };
        SelectionEvidence: {
            basis: string;
            method: string;
            reasons: components["schemas"]["SelectionReason"][];
            subject: string;
        };
        SelectionReason: {
            code: string;
            /** @description observation, limitation, or context; missing evidence is not a defect. */
            kind: string;
        };
        SelectiveEditSync: {
            /** Format: int64 */
            expected_revision: number;
            groups: components["schemas"]["EditSyncGroup"][];
            request_id: string;
            targets: components["schemas"]["BatchEditTarget"][];
        };
        SemanticEmbedding: {
            model_sha256: string;
            preprocessing: string;
            vector: number[];
        };
        SourceBinding: {
            fingerprint: string;
            /** Format: int32 */
            height: number;
            photo_id: string;
            /** @description None means camera as-shot WB. Explicit gains are sensor RGB, not Kelvin. */
            raw_white_balance?: number[] | null;
            reference_kind: components["schemas"]["ReferenceKind"];
            /** Format: int32 */
            width: number;
        };
        SourceRequest: {
            source: string;
        };
        ToneCurve: {
            /**
             * Format: float
             * @default 0
             */
            darks: number;
            /**
             * Format: float
             * @default 0
             */
            highlights: number;
            /**
             * Format: float
             * @default 0
             */
            lights: number;
            /**
             * Format: float
             * @description Log2 gains in four overlapping bands centred at Y=0, 1/3, 2/3, 1.
             * @default 0
             */
            shadows: number;
        };
        ToneMap: {
            /** @enum {string} */
            kind: "none";
        } | {
            /** @enum {string} */
            kind: "shoulder";
            /** Format: float */
            knee: number;
        };
        /** @enum {string} */
        Verdict: "recommend" | "review" | "reject_suggest";
        Vignette: {
            /**
             * Format: float
             * @description Edge exposure in stops [-4,4].
             * @default 0
             */
            amount: number;
            /**
             * Format: float
             * @description Falloff exponent [0.1,4].
             * @default 1
             */
            feather: number;
            /**
             * Format: float
             * @description Fraction of centre-to-corner radius at which falloff begins [0,0.99].
             * @default 0.3499999940395355
             */
            midpoint: number;
        };
        VisionAnalysis: {
            /** Format: double */
            composite_score: number;
            composition?: components["schemas"]["Composition"] | null;
            embedding?: components["schemas"]["SemanticEmbedding"] | null;
            exposure: components["schemas"]["Exposure"];
            faces: components["schemas"]["Face"][];
            /** Format: int32 */
            height: number;
            /** Format: double */
            niqe?: number | null;
            /** Format: int32 */
            orientation: number;
            /** Format: int32 */
            original_height: number;
            /** Format: int32 */
            original_width: number;
            phash: string;
            preview_source?: string | null;
            score_breakdown?: components["schemas"]["ScoreBreakdown"] | null;
            selection_evidence?: components["schemas"]["SelectionEvidence"] | null;
            /** Format: double */
            sharpness_fft: number;
            /** Format: double */
            sharpness_lap: number;
            structure: number[];
            verdict: components["schemas"]["Verdict"];
            version: string;
            warnings: string[];
            /** Format: int32 */
            width: number;
        };
        /** @enum {string} */
        WhiteBalanceLocus: "daylight" | "planckian";
        /** @description Coordinates in Iris's illuminant model, not vendor-specific temperature/tint. */
        WhiteBalanceTemperature: {
            /** Format: double */
            kelvin: number;
            locus: components["schemas"]["WhiteBalanceLocus"];
            /** Format: double */
            tint_delta_v: number;
        };
    };
    responses: never;
    parameters: never;
    requestBodies: never;
    headers: never;
    pathItems: never;
}
export type $defs = Record<string, never>;
export interface operations {
    bootstrap: {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Response */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["Bootstrap"];
                };
            };
        };
    };
    gpu_devices: {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Response */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["GpuDevices"];
                };
            };
            /** @description Session token required */
            401: {
                headers: {
                    [name: string]: unknown;
                };
                content?: never;
            };
        };
    };
    editing_list_presets: {
        parameters: {
            query?: {
                limit?: number;
                offset?: number;
            };
            header?: never;
            path?: never;
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Response */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["EditPreset"][];
                };
            };
        };
    };
    editing_create_preset: {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        requestBody: {
            content: {
                "application/json": components["schemas"]["PresetSettings"];
            };
        };
        responses: {
            /** @description Response */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["EditPreset"];
                };
            };
        };
    };
    editing_get_preset: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                id: string;
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Response */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["EditPreset"];
                };
            };
        };
    };
    editing_update_preset: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                id: string;
            };
            cookie?: never;
        };
        requestBody: {
            content: {
                "application/json": components["schemas"]["PresetSettings"];
            };
        };
        responses: {
            /** @description Response */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["EditPreset"];
                };
            };
        };
    };
    editing_delete_preset: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                id: string;
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Response */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["EditSuccess"];
                };
            };
        };
    };
    editing_preview_close: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                session: string;
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Response */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["EditSuccess"];
                };
            };
        };
    };
    editing_preview_cancel: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                session: string;
            };
            cookie?: never;
        };
        requestBody: {
            content: {
                "application/json": components["schemas"]["CancelRequest"];
            };
        };
        responses: {
            /** @description Response */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["EditSuccess"];
                };
            };
        };
    };
    editing_import_resource: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                kind: components["schemas"]["EditResourceKind"];
            };
            cookie?: never;
        };
        requestBody: {
            content: {
                "application/octet-stream": number[];
            };
        };
        responses: {
            /** @description Response */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["EditResource"];
                };
            };
            /** @description Resource exceeds byte limit */
            413: {
                headers: {
                    [name: string]: unknown;
                };
                content?: never;
            };
        };
    };
    editing_resource_bytes: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                hash: string;
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Response */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/octet-stream": number[];
                };
            };
        };
    };
    editing_capabilities: {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Response */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["EditingCapabilities"];
                };
            };
        };
    };
    editing_get_edit: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                id: string;
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Response */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["EditRecord"];
                };
            };
        };
    };
    editing_save_edit: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                id: string;
            };
            cookie?: never;
        };
        requestBody: {
            content: {
                "application/json": components["schemas"]["SaveEdit"];
            };
        };
        responses: {
            /** @description Response */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["EditRecord"];
                };
            };
        };
    };
    editing_delete_edit: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                id: string;
            };
            cookie?: never;
        };
        requestBody: {
            content: {
                "application/json": components["schemas"]["EditRevision"];
            };
        };
        responses: {
            /** @description Response */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["EditSuccess"];
                };
            };
        };
    };
    editing_batch_settings: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                id: string;
            };
            cookie?: never;
        };
        requestBody: {
            content: {
                "application/json": components["schemas"]["BatchSettings"];
            };
        };
        responses: {
            /** @description Response */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["EditRecord"][];
                };
            };
        };
    };
    editing_duplicate_edit: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                id: string;
            };
            cookie?: never;
        };
        requestBody: {
            content: {
                "application/json": components["schemas"]["EditName"];
            };
        };
        responses: {
            /** @description Response */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["EditRecord"];
                };
            };
        };
    };
    editing_preview_histogram: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                id: string;
            };
            cookie?: never;
        };
        requestBody: {
            content: {
                "application/json": components["schemas"]["HistogramRequest"];
            };
        };
        responses: {
            /** @description Response */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["HistogramResponse"];
                };
            };
        };
    };
    editing_edit_history: {
        parameters: {
            query?: {
                limit?: number;
                offset?: number;
            };
            header?: never;
            path: {
                id: string;
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Response */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["EditHistoryEntry"][];
                };
            };
        };
    };
    editing_calibration_lens_profile: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                id: string;
            };
            cookie?: never;
        };
        requestBody: {
            content: {
                "application/json": components["schemas"]["LensSuggestionRequest"];
            };
        };
        responses: {
            /** @description Response */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["LensProfileSuggestion"];
                };
            };
        };
    };
    editing_preview_preview: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                id: string;
            };
            cookie?: never;
        };
        requestBody: {
            content: {
                "application/json": components["schemas"]["RenderRequest"];
            };
        };
        responses: {
            /** @description Response */
            200: {
                headers: {
                    /** @description True when proxy evaluation was used */
                    "x-iris-approximate"?: boolean;
                    /** @description Nodes executed on CPU */
                    "x-iris-cpu-nodes"?: number;
                    /** @description Canonical rendering fingerprint */
                    "x-iris-fingerprint"?: string;
                    /** @description GPU attempt fell back to CPU */
                    "x-iris-gpu-fallback"?: boolean;
                    /** @description Nodes executed on GPU */
                    "x-iris-gpu-nodes"?: number;
                    /** @description cpu, gpu or mixed for this render */
                    "x-iris-render-backend"?: string;
                    /** @description Edit revision */
                    "x-iris-revision"?: number;
                    /** @description Render sequence */
                    "x-iris-sequence"?: number;
                    [name: string]: unknown;
                };
                content: {
                    "image/jpeg": number[];
                };
            };
        };
    };
    editing_preview_draft_preview: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                id: string;
            };
            cookie?: never;
        };
        requestBody: {
            content: {
                "application/json": components["schemas"]["DraftPreviewRequest"];
            };
        };
        responses: {
            /** @description Response */
            200: {
                headers: {
                    /** @description Always true; no revision saved */
                    "x-iris-draft"?: boolean;
                    /** @description Draft graph fingerprint */
                    "x-iris-fingerprint"?: string;
                    /** @description Saved base revision */
                    "x-iris-revision"?: number;
                    /** @description Latest-wins sequence */
                    "x-iris-sequence"?: number;
                    [name: string]: unknown;
                };
                content: {
                    "image/jpeg": number[];
                };
            };
        };
    };
    editing_preview_proof: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                id: string;
            };
            cookie?: never;
        };
        requestBody: {
            content: {
                "application/json": components["schemas"]["ProofRequest"];
            };
        };
        responses: {
            /** @description Response */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "image/jpeg": number[];
                };
            };
        };
    };
    editing_calibration_raw_white_balance: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                id: string;
            };
            cookie?: never;
        };
        requestBody: {
            content: {
                "application/json": components["schemas"]["PrepareWhiteBalanceRequest"];
            };
        };
        responses: {
            /** @description Response */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["PreparedRawWhiteBalance"];
                };
            };
        };
    };
    editing_export_recipe: {
        parameters: {
            query?: {
                include_masks?: boolean;
            };
            header?: never;
            path: {
                id: string;
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Response */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["EditRecipe"];
                };
            };
        };
    };
    editing_import_recipe: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                id: string;
            };
            cookie?: never;
        };
        requestBody: {
            content: {
                "application/json": components["schemas"]["ImportRecipe"];
            };
        };
        responses: {
            /** @description Response */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["EditRecord"];
                };
            };
        };
    };
    editing_redo_edit: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                id: string;
            };
            cookie?: never;
        };
        requestBody: {
            content: {
                "application/json": components["schemas"]["EditCommand"];
            };
        };
        responses: {
            /** @description Response */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["EditRecord"];
                };
            };
        };
    };
    editing_preview_region: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                id: string;
            };
            cookie?: never;
        };
        requestBody: {
            content: {
                "application/json": components["schemas"]["RegionRequest"];
            };
        };
        responses: {
            /** @description Response */
            200: {
                headers: {
                    /** @description Always false: native output pixels */
                    "x-iris-approximate"?: boolean;
                    /** @description Canonical rendering fingerprint */
                    "x-iris-fingerprint"?: string;
                    /** @description True: evaluates entire image before crop */
                    "x-iris-full-frame-evaluation"?: boolean;
                    /** @description Full edited output height */
                    "x-iris-full-height"?: number;
                    /** @description Full edited output width */
                    "x-iris-full-width"?: number;
                    /** @description Native region height */
                    "x-iris-region-height"?: number;
                    /** @description Native region width */
                    "x-iris-region-width"?: number;
                    /** @description Output pixel origin x */
                    "x-iris-region-x"?: number;
                    /** @description Output pixel origin y */
                    "x-iris-region-y"?: number;
                    /** @description Edit revision */
                    "x-iris-revision"?: number;
                    /** @description Render sequence */
                    "x-iris-sequence"?: number;
                    [name: string]: unknown;
                };
                content: {
                    "image/jpeg": number[];
                };
            };
        };
    };
    editing_preview_sample: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                id: string;
            };
            cookie?: never;
        };
        requestBody: {
            content: {
                "application/json": components["schemas"]["SampleRequest"];
            };
        };
        responses: {
            /** @description Response */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["SampleResponse"];
                };
            };
        };
    };
    editing_sync_settings: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                id: string;
            };
            cookie?: never;
        };
        requestBody: {
            content: {
                "application/json": components["schemas"]["SelectiveEditSync"];
            };
        };
        responses: {
            /** @description Response */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["EditRecord"][];
                };
            };
        };
    };
    editing_undo_edit: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                id: string;
            };
            cookie?: never;
        };
        requestBody: {
            content: {
                "application/json": components["schemas"]["EditCommand"];
            };
        };
        responses: {
            /** @description Response */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["EditRecord"];
                };
            };
        };
    };
    models: {
        parameters: {
            query: {
                project_id: number;
            };
            header?: never;
            path?: never;
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Response */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ModelStatusResponse"];
                };
            };
        };
    };
    status: {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Response */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["InstallProgress"];
                };
            };
        };
    };
    start: {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        requestBody: {
            content: {
                "application/json": components["schemas"]["InstallRequest"];
            };
        };
        responses: {
            /** @description Response */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["InstallProgress"];
                };
            };
        };
    };
    cancel_model_install: {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Response */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["InstallProgress"];
                };
            };
        };
    };
    catalog: {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Response */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["OptionalModel"][];
                };
            };
        };
    };
    photo: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                id: number;
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Response */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["Photo"];
                };
            };
        };
    };
    detail: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                id: number;
            };
            cookie?: never;
        };
        requestBody: {
            content: {
                "application/json": components["schemas"]["DetailRequest"];
            };
        };
        responses: {
            /** @description Response */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["DetailEvidence"];
                };
            };
        };
    };
    editing_list_edits: {
        parameters: {
            query?: {
                limit?: number;
                offset?: number;
            };
            header?: never;
            path: {
                id: number;
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Response */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["EditRecord"][];
                };
            };
        };
    };
    editing_create_edit: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                id: number;
            };
            cookie?: never;
        };
        requestBody: {
            content: {
                "application/json": components["schemas"]["EditName"];
            };
        };
        responses: {
            /** @description Response */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["EditRecord"];
                };
            };
        };
    };
    photo_metadata: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                id: number;
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Response */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["PhotoMetadata"];
                };
            };
        };
    };
    original: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                id: number;
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Unmodified source bytes; RAW and HEIC review uses JPEG previews */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "image/jpeg": number[];
                    "image/png": number[];
                    "image/webp": number[];
                    "image/tiff": number[];
                    "image/heic": number[];
                    "application/octet-stream": number[];
                };
            };
        };
    };
    preview: {
        parameters: {
            query?: {
                /** @description Low-priority speculative preview */
                prefetch?: boolean;
            };
            header?: never;
            path: {
                id: number;
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description JPEG preview */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "image/jpeg": number[];
                };
            };
        };
    };
    thumb: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                id: number;
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description JPEG thumbnail */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "image/jpeg": number[];
                };
            };
        };
    };
    profiles: {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Response */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["Profile"][];
                };
            };
        };
    };
    save_profile: {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        requestBody: {
            content: {
                "application/json": components["schemas"]["Profile"];
            };
        };
        responses: {
            /** @description Response */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["Profile"];
                };
            };
        };
    };
    delete_profile: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                name: string;
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Response */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "text/plain": boolean;
                };
            };
        };
    };
    apply_profile: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                name: string;
            };
            cookie?: never;
        };
        requestBody: {
            content: {
                "application/json": components["schemas"]["ProjectRequest"];
            };
        };
        responses: {
            /** @description Response */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["AnalysisSettings"];
                };
            };
        };
    };
    projects: {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Response */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["Project"][];
                };
            };
        };
    };
    create_project: {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        requestBody: {
            content: {
                "application/json": components["schemas"]["CreateProject"];
            };
        };
        responses: {
            /** @description Response */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["Project"];
                };
            };
        };
    };
    project: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                id: number;
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Response */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["Project"];
                };
            };
        };
    };
    accept: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                id: number;
            };
            cookie?: never;
        };
        requestBody?: {
            content: {
                "application/json": components["schemas"]["AcceptRequest"] | null;
            };
        };
        responses: {
            /** @description Response */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["DecisionBatch"];
                };
            };
        };
    };
    analyze: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                id: number;
            };
            cookie?: never;
        };
        requestBody?: {
            content: {
                "application/json": components["schemas"]["AnalyzeRequest"] | null;
            };
        };
        responses: {
            /** @description Response */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["JobProgress"];
                };
            };
        };
    };
    browse_context: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                id: number;
            };
            cookie?: never;
        };
        requestBody: {
            content: {
                "application/json": components["schemas"]["BrowseContext"];
            };
        };
        responses: {
            /** @description Response */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["BrowseContext"];
                };
            };
        };
    };
    cache_status: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                id: number;
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Response */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["CacheStatus"];
                };
            };
        };
    };
    cache_cleanup: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                id: number;
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Response */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["CacheStatus"];
                };
            };
        };
    };
    cache_migrate: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                id: number;
            };
            cookie?: never;
        };
        requestBody: {
            content: {
                "application/json": components["schemas"]["DestinationRequest"];
            };
        };
        responses: {
            /** @description Response */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["CacheStatus"];
                };
            };
        };
    };
    cache_migrations: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                id: number;
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Response */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["CacheMigration"][];
                };
            };
        };
    };
    cache_cleanup_old: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                id: number;
                migration_id: string;
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Response */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["CacheMigration"];
                };
            };
        };
    };
    cancel: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                id: number;
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Response */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["JobProgress"];
                };
            };
        };
    };
    compare_photos: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                id: number;
            };
            cookie?: never;
        };
        requestBody: {
            content: {
                "application/json": components["schemas"]["ComparisonRequest"];
            };
        };
        responses: {
            /** @description Response */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ComparisonReport"];
                };
            };
        };
    };
    decisions: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                id: number;
            };
            cookie?: never;
        };
        requestBody: {
            content: {
                "application/json": components["schemas"]["DecisionRequest"];
            };
        };
        responses: {
            /** @description Response */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["DecisionBatch"];
                };
            };
        };
    };
    export_copy: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                id: number;
            };
            cookie?: never;
        };
        requestBody: {
            content: {
                "application/json": components["schemas"]["ExportRequest"];
            };
        };
        responses: {
            /** @description Response */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ExportReport"];
                };
            };
        };
    };
    export_csv: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                id: number;
            };
            cookie?: never;
        };
        requestBody: {
            content: {
                "application/json": components["schemas"]["DestinationRequest"];
            };
        };
        responses: {
            /** @description Response */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ExportReport"];
                };
            };
        };
    };
    export_xmp: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                id: number;
            };
            cookie?: never;
        };
        requestBody: {
            content: {
                "application/json": components["schemas"]["ExportRequest"];
            };
        };
        responses: {
            /** @description Response */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ExportReport"];
                };
            };
        };
    };
    groups: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                id: number;
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Response */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["BurstGroup"][];
                };
            };
        };
    };
    group_selection: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                id: number;
                group_id: string;
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Response */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["GroupSelection"];
                };
            };
        };
    };
    set_group_intent: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                id: number;
                group_id: string;
            };
            cookie?: never;
        };
        requestBody: {
            content: {
                "application/json": components["schemas"]["GroupIntent"];
            };
        };
        responses: {
            /** @description Response */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["GroupSelection"];
                };
            };
        };
    };
    accept_group_selection: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                id: number;
                group_id: string;
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Response */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["DecisionBatch"];
                };
            };
        };
    };
    hide_project: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                id: number;
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Response */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["Project"];
                };
            };
        };
    };
    import_csv: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                id: number;
            };
            cookie?: never;
        };
        requestBody: {
            content: {
                "application/json": components["schemas"]["SourceRequest"];
            };
        };
        responses: {
            /** @description Response */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["DecisionBatch"];
                };
            };
        };
    };
    marks: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                id: number;
            };
            cookie?: never;
        };
        requestBody: {
            content: {
                "application/json": components["schemas"]["MarkRequest"];
            };
        };
        responses: {
            /** @description Response */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["DecisionBatch"];
                };
            };
        };
    };
    open_project: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                id: number;
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Response */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["Project"];
                };
            };
        };
    };
    pause: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                id: number;
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Response */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["JobProgress"];
                };
            };
        };
    };
    photos: {
        parameters: {
            query?: {
                decision?: components["schemas"]["Action"];
                rating?: number;
                color_label?: components["schemas"]["ColorLabel"];
                verdict?: string;
                format?: string;
                sort?: string;
                descending?: boolean;
                include_missing?: boolean;
                offset?: number;
                limit?: number;
            };
            header?: never;
            path: {
                id: number;
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Response */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["Photo"][];
                };
            };
        };
    };
    estimate_profile: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                id: number;
                name: string;
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Response */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": unknown;
                };
            };
        };
    };
    progress: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                id: number;
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Response */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["JobProgress"];
                };
            };
        };
    };
    quarantine_history: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                id: number;
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Response */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["QuarantinePlan"][];
                };
            };
        };
    };
    quarantine_commit: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                id: number;
            };
            cookie?: never;
        };
        requestBody: {
            content: {
                "application/json": components["schemas"]["ManifestRequest"];
            };
        };
        responses: {
            /** @description Response */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["QuarantinePlan"];
                };
            };
        };
    };
    quarantine_preview: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                id: number;
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Response */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["QuarantinePlan"];
                };
            };
        };
    };
    quarantine_restore: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                id: number;
            };
            cookie?: never;
        };
        requestBody: {
            content: {
                "application/json": components["schemas"]["ManifestRequest"];
            };
        };
        responses: {
            /** @description Response */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["QuarantinePlan"];
                };
            };
        };
    };
    resume: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                id: number;
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Response */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["JobProgress"];
                };
            };
        };
    };
    scan: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                id: number;
            };
            cookie?: never;
        };
        requestBody?: {
            content: {
                "application/json": components["schemas"]["ScanRequest"] | null;
            };
        };
        responses: {
            /** @description Response */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["JobProgress"];
                };
            };
        };
    };
    scenes: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                id: number;
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Response */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["SceneGroup"][];
                };
            };
        };
    };
    correct_scene: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                id: number;
            };
            cookie?: never;
        };
        requestBody: {
            content: {
                "application/json": components["schemas"]["SceneCorrection"];
            };
        };
        responses: {
            /** @description Response */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["SceneGroup"][];
                };
            };
        };
    };
    search: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                id: number;
            };
            cookie?: never;
        };
        requestBody: {
            content: {
                "application/json": components["schemas"]["SearchRequest"];
            };
        };
        responses: {
            /** @description Response */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["SearchResponse"];
                };
            };
        };
    };
    index_search: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                id: number;
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Response */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["JobProgress"];
                };
            };
        };
    };
    search_status: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                id: number;
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Response */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["SearchStatus"];
                };
            };
        };
    };
    undo: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                id: number;
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Response */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["DecisionBatch"];
                };
            };
        };
    };
    editing_exports_list: {
        parameters: {
            query?: {
                limit?: number;
                offset?: number;
            };
            header?: never;
            path?: never;
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Response */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["RenderedExportStatus"][];
                };
            };
        };
    };
    editing_exports_queue: {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        requestBody: {
            content: {
                "application/json": components["schemas"]["QueueExports"];
            };
        };
        responses: {
            /** @description Response */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["RenderedExportStatus"][];
                };
            };
        };
    };
    editing_exports_status: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                id: string;
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Response */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["RenderedExportStatus"];
                };
            };
        };
    };
    editing_exports_delete: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                id: string;
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Response */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["EditSuccess"];
                };
            };
        };
    };
    editing_exports_cancel: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                id: string;
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Response */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["RenderedExportStatus"];
                };
            };
        };
    };
    editing_exports_recover: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                id: string;
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Response */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["RenderedExportStatus"];
                };
            };
        };
    };
    editing_exports_retry: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                id: string;
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Response */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["RenderedExportStatus"];
                };
            };
        };
    };
    settings: {
        parameters: {
            query: {
                project_id: number;
            };
            header?: never;
            path?: never;
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Response */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["AnalysisSettings"];
                };
            };
        };
    };
    put_settings: {
        parameters: {
            query: {
                project_id: number;
            };
            header?: never;
            path?: never;
            cookie?: never;
        };
        requestBody: {
            content: {
                "application/json": components["schemas"]["AnalysisSettings"];
            };
        };
        responses: {
            /** @description Response */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["AnalysisSettings"];
                };
            };
        };
    };
}
