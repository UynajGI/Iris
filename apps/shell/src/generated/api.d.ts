// Generated from iris-daemon OpenAPI. Do not edit.
// Contract SHA-256: 5a2dc12ec80b29f965c0ba7771d712abde521cb570e326d8a0339e5f5fa9b37e
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
        Bootstrap: {
            capabilities: string[];
            recovery_notice?: string | null;
            version: string;
        };
        BurstGroup: {
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
        /** @enum {string} */
        ColorLabel: "none" | "red" | "yellow" | "green" | "blue" | "purple";
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
        /** @description `available` means file/hash/metadata checks passed, not inference or accuracy validation. */
        DetectorModelStatus: {
            provider: components["schemas"]["FaceDetectorProvider"];
            reason?: string | null;
            /** @description Actual SHA-256 of bytes read; null when no model bytes could be inspected. */
            sha256?: string | null;
            state: components["schemas"]["ModelAvailability"];
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
            head_pose?: null | components["schemas"]["HeadPose"];
            index: number;
            /** Format: float */
            landmark_confidence?: number | null;
            landmarks: number[][];
            left_eye?: null | components["schemas"]["Eye"];
            left_eye_unreliable_reason?: string | null;
            left_eye_visibility?: null | components["schemas"]["EyeVisibility"];
            quality?: null | components["schemas"]["FaceQuality"];
            quality_unavailable_reason?: string | null;
            right_eye?: null | components["schemas"]["Eye"];
            right_eye_unreliable_reason?: string | null;
            right_eye_visibility?: null | components["schemas"]["EyeVisibility"];
            /** Format: float */
            smile_score?: number | null;
            unreliable_reason?: string | null;
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
        HeadPose: {
            method: string;
            /** Format: float */
            pitch: number;
            /** Format: float */
            roll: number;
            /** Format: float */
            yaw: number;
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
            previous_execution?: components["schemas"]["ExecutionFeedback"][];
            result?: unknown;
            root_unavailable?: boolean;
            state: string;
            total: number;
        };
        ManifestRequest: {
            manifest_id: string;
        };
        MarkRequest: {
            color_label?: null | components["schemas"]["ColorLabel"];
            decision?: null | components["schemas"]["Action"];
            photo_ids: number[];
            /** Format: int32 */
            rating?: number | null;
        };
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
        OcclusionModelStatus: {
            provider: components["schemas"]["OcclusionProvider"];
            reason?: string | null;
            /** @description Actual hash of read bytes. Available only describes artifact checks, not inference or licensing. */
            sha256?: string | null;
            state: components["schemas"]["ModelAvailability"];
        };
        /** @enum {string} */
        OcclusionProvider: "none" | "faceocc";
        OptionalModel: {
            bytes: number;
            id: string;
            license: string;
            license_url: string;
            sha256: string;
            state: string;
            title: string;
        };
        Photo: {
            analysis?: null | components["schemas"]["VisionAnalysis"];
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
            color_label?: null | components["schemas"]["ColorLabel"];
            decision?: null | components["schemas"]["Action"];
            descending?: boolean | null;
            format?: string | null;
            include_missing?: boolean | null;
            limit?: number | null;
            offset?: number | null;
            /** Format: int32 */
            rating?: number | null;
            sort?: string | null;
            verdict?: string | null;
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
        ScanRequest: {
            /** @default false */
            retry_failed_only: boolean;
        };
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
        SemanticEmbedding: {
            model_sha256: string;
            preprocessing: string;
            vector: number[];
        };
        SourceRequest: {
            source: string;
        };
        /** @enum {string} */
        Verdict: "recommend" | "review" | "reject_suggest";
        VisionAnalysis: {
            /** Format: double */
            composite_score: number;
            composition?: null | components["schemas"]["Composition"];
            embedding?: null | components["schemas"]["SemanticEmbedding"];
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
            score_breakdown?: null | components["schemas"]["ScoreBreakdown"];
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
                    "image/heic": number[];
                    "application/octet-stream": number[];
                };
            };
        };
    };
    preview: {
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
                "application/json": null | components["schemas"]["AcceptRequest"];
            };
        };
        responses: {
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
                "application/json": null | components["schemas"]["AnalyzeRequest"];
            };
        };
        responses: {
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
                "application/json": null | components["schemas"]["ScanRequest"];
            };
        };
        responses: {
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
