package com.golammowla.zrdownloader.di

import android.content.Context
import androidx.room.Room
import com.golammowla.zrdownloader.data.database.ZrDatabase
import com.golammowla.zrdownloader.data.database.dao.DownloadDao
import dagger.Module
import dagger.Provides
import dagger.hilt.InstallIn
import dagger.hilt.android.qualifiers.ApplicationContext
import dagger.hilt.components.SingletonComponent
import javax.inject.Singleton

@Module
@InstallIn(SingletonComponent::class)
object DatabaseModule {

    @Provides
    @Singleton
    fun provideDatabase(@ApplicationContext context: Context): ZrDatabase {
        return Room.databaseBuilder(
            context,
            ZrDatabase::class.java,
            ZrDatabase.DATABASE_NAME
        )
        .fallbackToDestructiveMigration()
        .build()
    }

    @Provides
    fun provideDownloadDao(database: ZrDatabase): DownloadDao {
        return database.downloadDao()
    }
}

