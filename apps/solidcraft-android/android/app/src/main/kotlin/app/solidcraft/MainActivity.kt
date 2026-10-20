package app.solidcraft

import android.content.Intent
import android.net.Uri
import android.provider.OpenableColumns
import com.google.androidgamesdk.GameActivity
import java.io.File

class MainActivity : GameActivity() {
    fun openDocument() {
        val intent = Intent(Intent.ACTION_OPEN_DOCUMENT).apply {
            addCategory(Intent.CATEGORY_OPENABLE)
            type = "*/*"
        }
        startActivityForResult(intent, REQUEST_OPEN)
    }

    @Deprecated("Activity result API is not required for this single request")
    override fun onActivityResult(requestCode: Int, resultCode: Int, data: Intent?) {
        super.onActivityResult(requestCode, resultCode, data)
        if (requestCode != REQUEST_OPEN) {
            return
        }
        val uri = data?.data
        val path = if (resultCode == RESULT_OK && uri != null) copyToFiles(uri) else ""
        nativePickerResult(path)
    }

    private fun copyToFiles(uri: Uri): String {
        val name = displayName(uri) ?: "import.solidcraft"
        val safe = name.replace(Regex("[^A-Za-z0-9._-]"), "_").take(80).ifEmpty { "import.solidcraft" }
        val dest = File(filesDir, safe)
        val input = contentResolver.openInputStream(uri) ?: return ""
        input.use { src ->
            dest.outputStream().use { out -> src.copyTo(out) }
        }
        return dest.absolutePath
    }

    private fun displayName(uri: Uri): String? {
        val cursor = contentResolver.query(uri, arrayOf(OpenableColumns.DISPLAY_NAME), null, null, null) ?: return null
        cursor.use {
            if (!it.moveToFirst()) {
                return null
            }
            val index = it.getColumnIndex(OpenableColumns.DISPLAY_NAME)
            if (index < 0) {
                return null
            }
            return it.getString(index)
        }
    }

    private external fun nativePickerResult(path: String)

    companion object {
        private const val REQUEST_OPEN = 1
    }
}
