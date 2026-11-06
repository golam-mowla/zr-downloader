package com.golammowla.zrdownloader.data.database

import androidx.room.Database
import androidx.room.RoomDatabase
import com.golammowla.zrdownloader.data.database.dao.DownloadDao
import com.golammowla.zrdownloader.data.database.entity.DownloadEntity

@Database(entities = [DownloadEntity::class], version = 1, exportSchema = false)
abstract class ZrDatabase : RoomDatabase() {
    abstract fun downloadDao(): DownloadDao

    companion object {
        const val DATABASE_NAME = "zr_downloads.db"
    }
}

